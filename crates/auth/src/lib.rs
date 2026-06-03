//! Authentication, API keys, and request scoping for AccelerateSearch.
//!
//! Two layers of authentication are supported:
//!
//! * **Master key** – a single, global API key configured via
//!   `auth.master_key`. The master key grants all permissions.
//! * **API keys** – first-class objects stored in the storage layer. Each
//!   has scoped permissions, optional expiry, and optional collection scope.
//!
//! Keys are stored in the storage layer as SHA-256 hashes of the secret
//! material. The plaintext key is only ever shown to the user at creation
//! time.

use std::sync::Arc;

use actix_web::Error as ActixError;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::http::header::AUTHORIZATION;
use chrono::{DateTime, Utc};
use futures_util::future::{LocalBoxFuture, Ready, ready};
use models::{ApiKey, ApiKeyId, CollectionId, Permission};
use serde::{Deserialize, Serialize};
use storage::StorageBackend;
use utils::{generate_api_key, sha256_hex};

use errors::{AppError, AppResult};

/// Storage table used for API keys.
pub const TABLE_KEYS: &str = storage::TABLE_KEYS;

/// Authenticated principal extracted from the request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Principal {
    /// UID of the API key (master key uses a special UID).
    pub key_uid: ApiKeyId,
    /// Human-readable name of the key.
    pub name: String,
    /// Permissions granted to the key.
    pub actions: Vec<Permission>,
    /// Collections the key is scoped to. `None` means all.
    pub indexes: Option<Vec<CollectionId>>,
}

impl Principal {
    /// Returns true if the principal has the admin wildcard.
    #[must_use]
    pub fn is_admin(&self) -> bool {
        self.actions.iter().any(|p| p.is_admin())
    }

    /// Returns true if the principal has the given permission.
    #[must_use]
    pub fn has_permission(&self, p: Permission) -> bool {
        self.is_admin() || self.actions.contains(&p)
    }

    /// Returns true if the principal can act on the given collection.
    #[must_use]
    pub fn can_act_on(&self, collection: &CollectionId) -> bool {
        match &self.indexes {
            None => true,
            Some(list) => list.iter().any(|c| c == collection),
        }
    }
}

/// Service that handles API key creation, lookup, and validation.
pub struct AuthService {
    storage: Arc<dyn StorageBackend>,
    master_key_hash: String,
    /// Cache of `key_hash -> Principal` to avoid hitting storage on every
    /// request. Invalidated on key update/delete.
    cache: dashmap::DashMap<String, (Principal, DateTime<Utc>)>,
}

impl AuthService {
    /// Creates a new `AuthService`.
    pub fn new(storage: Arc<dyn StorageBackend>, master_key: &str) -> Self {
        let master_key_hash = if master_key.is_empty() {
            String::new()
        } else {
            sha256_hex(master_key)
        };
        Self {
            storage,
            master_key_hash,
            cache: dashmap::DashMap::new(),
        }
    }

    /// Returns true if authentication is enabled (master key configured).
    #[must_use]
    pub fn is_auth_enabled(&self) -> bool {
        !self.master_key_hash.is_empty()
    }

    /// Creates a new API key.
    pub async fn create_key(
        &self,
        name: &str,
        description: Option<String>,
        actions: Vec<Permission>,
        indexes: Option<Vec<CollectionId>>,
        expires_at: Option<DateTime<Utc>>,
    ) -> AppResult<(ApiKey, String)> {
        let uid = ApiKeyId::generate();
        let key = generate_api_key();
        let key_hash = sha256_hex(&key);
        let key_prefix = key.chars().take(8).collect::<String>();
        let now = Utc::now();
        let api_key = ApiKey {
            uid,
            name: name.to_string(),
            description,
            key_hash: key_hash.clone(),
            key_prefix,
            actions,
            indexes,
            expires_at,
            created_at: now,
            updated_at: now,
        };
        let bytes = serde_json::to_vec(&api_key)?;
        let uid_str = uid.to_string();
        self.storage.put(TABLE_KEYS, &uid_str, bytes).await?;
        self.cache
            .insert(key_hash, (self.principal_for(&api_key), now));
        Ok((api_key, key))
    }

    /// Returns a key by UID.
    pub async fn get_key(&self, uid: ApiKeyId) -> AppResult<Option<ApiKey>> {
        let key = uid.to_string();
        match self.storage.get(TABLE_KEYS, &key).await? {
            Some(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            None => Ok(None),
        }
    }

    /// Lists all API keys.
    pub async fn list_keys(&self) -> AppResult<Vec<ApiKey>> {
        let keys = self.storage.list(TABLE_KEYS, "").await?;
        let mut out = Vec::with_capacity(keys.len());
        for k in keys {
            if let Some(bytes) = self.storage.get(TABLE_KEYS, &k).await? {
                let api_key: ApiKey = serde_json::from_slice(&bytes)?;
                out.push(api_key);
            }
        }
        Ok(out)
    }

    /// Updates an existing key.
    pub async fn update_key(
        &self,
        uid: ApiKeyId,
        name: Option<String>,
        description: Option<Option<String>>,
        actions: Option<Vec<Permission>>,
        indexes: Option<Option<Vec<CollectionId>>>,
        expires_at: Option<Option<DateTime<Utc>>>,
    ) -> AppResult<ApiKey> {
        let mut existing = self
            .get_key(uid)
            .await?
            .ok_or_else(|| AppError::not_found(format!("API key {} not found", uid)))?;
        if let Some(n) = name {
            existing.name = n;
        }
        if let Some(d) = description {
            existing.description = d;
        }
        if let Some(a) = actions {
            existing.actions = a;
        }
        if let Some(i) = indexes {
            existing.indexes = i;
        }
        if let Some(e) = expires_at {
            existing.expires_at = e;
        }
        existing.updated_at = Utc::now();
        let bytes = serde_json::to_vec(&existing)?;
        self.storage
            .put(TABLE_KEYS, &uid.to_string(), bytes)
            .await?;
        self.cache.remove(&existing.key_hash);
        Ok(existing)
    }

    /// Deletes an API key.
    pub async fn delete_key(&self, uid: ApiKeyId) -> AppResult<bool> {
        let existing = self.get_key(uid).await?;
        let removed = self.storage.delete(TABLE_KEYS, &uid.to_string()).await?;
        if let Some(k) = existing {
            self.cache.remove(&k.key_hash);
        }
        Ok(removed)
    }

    /// Authenticates a request by extracting the bearer token from the
    /// `Authorization` header and looking up the matching key.
    pub async fn authenticate(&self, token: &str) -> AppResult<Principal> {
        let token_hash = sha256_hex(token);
        if !self.master_key_hash.is_empty() && token_hash == self.master_key_hash {
            return Ok(self.master_principal());
        }
        if let Some(entry) = self.cache.get(&token_hash) {
            let (p, _ts) = entry.value();
            if let Some(exp) = self.fetch_key_expiry(p.key_uid).await?
                && exp < Utc::now()
            {
                drop(entry);
                self.cache.remove(&token_hash);
                return Err(AppError::Unauthorized("API key expired".into()));
            }
            return Ok(p.clone());
        }
        // Slow path: scan the keys table (small, this is fine).
        for key in self.list_keys().await? {
            if key.key_hash == token_hash {
                if let Some(exp) = key.expires_at
                    && exp < Utc::now()
                {
                    return Err(AppError::Unauthorized("API key expired".into()));
                }
                let principal = self.principal_for(&key);
                self.cache
                    .insert(token_hash, (principal.clone(), Utc::now()));
                return Ok(principal);
            }
        }
        Err(AppError::Unauthorized("invalid API key".into()))
    }

    fn principal_for(&self, key: &ApiKey) -> Principal {
        Principal {
            key_uid: key.uid,
            name: key.name.clone(),
            actions: key.actions.clone(),
            indexes: key.indexes.clone(),
        }
    }

    fn master_principal(&self) -> Principal {
        Principal {
            key_uid: ApiKeyId::from_uuid(uuid::Uuid::nil()),
            name: "master".into(),
            actions: vec![Permission::All],
            indexes: None,
        }
    }

    async fn fetch_key_expiry(&self, uid: ApiKeyId) -> AppResult<Option<DateTime<Utc>>> {
        match self.get_key(uid).await? {
            Some(k) => Ok(k.expires_at),
            None => Ok(None),
        }
    }

    /// Clears the in-memory cache (useful for tests).
    pub fn clear_cache(&self) {
        self.cache.clear();
    }
}

/// Middleware factory that authenticates each incoming request.
#[derive(Clone)]
pub struct AuthMiddleware {
    service: Arc<AuthService>,
}

impl AuthMiddleware {
    /// Creates a new middleware backed by the given `AuthService`.
    #[must_use]
    pub fn new(service: Arc<AuthService>) -> Self {
        Self { service }
    }
}

impl<S, B> Transform<S, ServiceRequest> for AuthMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = ActixError> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = ActixError;
    type Transform = AuthMiddlewareInner<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AuthMiddlewareInner {
            service,
            auth: self.service.clone(),
        }))
    }
}

pub struct AuthMiddlewareInner<S> {
    service: S,
    auth: Arc<AuthService>,
}

impl<S, B> Service<ServiceRequest> for AuthMiddlewareInner<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = ActixError> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = ActixError;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    actix_web::dev::forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let path = req.path().to_string();

        if is_public_path(&path) || !self.auth.is_auth_enabled() {
            let fut = self.service.call(req);
            return Box::pin(fut);
        }

        let token = req
            .headers()
            .get(AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .map(str::to_owned);

        let auth = self.auth.clone();
        let fut = self.service.call(req);

        Box::pin(async move {
            let token = match token {
                Some(t) => t,
                None => return Err(actix_web::error::ErrorUnauthorized("missing bearer token")),
            };
            match auth.authenticate(&token).await {
                Ok(_principal) => fut.await,
                Err(e) => Err(actix_web::error::ErrorUnauthorized(e.to_string())),
            }
        })
    }
}

/// Returns true if `path` does not require authentication.
#[must_use]
pub fn is_public_path(path: &str) -> bool {
    matches!(
        path,
        "/health"
            | "/version"
            | "/metrics"
            | "/swagger-ui"
            | "/swagger-ui/"
            | "/api-docs/openapi.json"
    ) || path.starts_with("/swagger-ui/")
        || path.starts_with("/api-docs/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::RedbStorage;

    async fn new_service() -> Arc<AuthService> {
        let backend = Arc::new(RedbStorage::open_temp().unwrap());
        Arc::new(AuthService::new(backend, "masterkey"))
    }

    #[tokio::test]
    async fn create_and_authenticate_key() {
        let svc = new_service().await;
        let (api_key, plaintext) = svc
            .create_key(
                "frontend",
                None,
                vec![Permission::Search, Permission::DocumentsAdd],
                None,
                None,
            )
            .await
            .unwrap();
        assert_eq!(api_key.name, "frontend");
        let principal = svc.authenticate(&plaintext).await.unwrap();
        assert!(principal.has_permission(Permission::Search));
        assert!(!principal.has_permission(Permission::KeysDelete));
    }

    #[tokio::test]
    async fn master_key_authenticates() {
        let svc = new_service().await;
        let p = svc.authenticate("masterkey").await.unwrap();
        assert!(p.is_admin());
    }

    #[tokio::test]
    async fn invalid_token_rejected() {
        let svc = new_service().await;
        assert!(svc.authenticate("nope").await.is_err());
    }

    #[tokio::test]
    async fn expired_key_rejected() {
        let svc = new_service().await;
        let (_, plaintext) = svc
            .create_key(
                "expiring",
                None,
                vec![Permission::Search],
                None,
                Some(Utc::now() - chrono::Duration::seconds(1)),
            )
            .await
            .unwrap();
        let err = svc.authenticate(&plaintext).await.unwrap_err();
        assert_eq!(err.code(), "unauthorized");
    }

    #[tokio::test]
    async fn scoped_indexes_enforced() {
        let svc = new_service().await;
        let (_, plaintext) = svc
            .create_key(
                "scoped",
                None,
                vec![Permission::Search],
                Some(vec![CollectionId::new("products")]),
                None,
            )
            .await
            .unwrap();
        let p = svc.authenticate(&plaintext).await.unwrap();
        assert!(p.can_act_on(&CollectionId::new("products")));
        assert!(!p.can_act_on(&CollectionId::new("other")));
    }

    #[tokio::test]
    async fn delete_key_removes_from_cache() {
        let svc = new_service().await;
        let (api_key, plaintext) = svc
            .create_key("tmp", None, vec![Permission::Search], None, None)
            .await
            .unwrap();
        assert!(svc.authenticate(&plaintext).await.is_ok());
        svc.delete_key(api_key.uid).await.unwrap();
        assert!(svc.authenticate(&plaintext).await.is_err());
    }

    #[test]
    fn public_path_detection() {
        assert!(is_public_path("/health"));
        assert!(is_public_path("/version"));
        assert!(is_public_path("/metrics"));
        assert!(is_public_path("/swagger-ui"));
        assert!(!is_public_path("/api/v1/collections"));
    }
}
