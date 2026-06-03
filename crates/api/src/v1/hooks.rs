//! Webhook / hooks endpoints.
//!
//! Hooks let clients subscribe to asynchronous events (e.g. document
//! ingestion completion, settings changes) and receive an HTTP POST at a
//! target URL when those events occur. Each hook is persisted in the
//! storage layer under [`TABLE_HOOKS`].

use std::collections::BTreeMap;
use std::sync::Arc;

use actix_web::{HttpResponse, ResponseError, delete, get, patch, post, web};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tracing::info;
use uuid::Uuid;

use errors::{AppError, AppResult};
use models::CollectionId;
use storage::{StorageBackend, put_json};

use crate::state::AppState;

/// Storage table used for hooks.
pub const TABLE_HOOKS: &str = "hooks";

/// In-memory hooks registry, also persisted in storage.
pub struct HookService {
    storage: Arc<dyn StorageBackend>,
    cache: DashMap<Uuid, Hook>,
}

impl HookService {
    /// Creates a new hook service.
    #[must_use]
    pub fn new(storage: Arc<dyn StorageBackend>) -> Self {
        Self {
            storage,
            cache: DashMap::new(),
        }
    }

    /// Loads all hooks from storage.
    pub async fn load_all(&self) -> AppResult<()> {
        self.cache.clear();
        for k in self.storage.list(TABLE_HOOKS, "").await? {
            if let Some(bytes) = self.storage.get(TABLE_HOOKS, &k).await? {
                let h: Hook = serde_json::from_slice(&bytes)?;
                self.cache.insert(h.id, h);
            }
        }
        Ok(())
    }

    /// Lists all hooks.
    #[must_use]
    pub fn list(&self) -> Vec<Hook> {
        self.cache.iter().map(|kv| kv.value().clone()).collect()
    }

    /// Returns a hook by id.
    #[must_use]
    pub fn get(&self, id: Uuid) -> Option<Hook> {
        self.cache.get(&id).map(|h| h.value().clone())
    }

    /// Creates a new hook.
    pub async fn create(&self, mut hook: Hook) -> AppResult<Hook> {
        if hook.id.is_nil() {
            hook.id = Uuid::new_v4();
        }
        let now = Utc::now();
        hook.created_at = now;
        hook.updated_at = now;
        self.persist(&hook).await?;
        self.cache.insert(hook.id, hook.clone());
        info!(hook = %hook.id, "created hook");
        Ok(hook)
    }

    /// Updates a hook.
    pub async fn update(&self, id: Uuid, patch: HookPatch) -> AppResult<Option<Hook>> {
        let Some(mut h) = self.get(id) else {
            return Ok(None);
        };
        if let Some(name) = patch.name {
            h.name = name;
        }
        if let Some(url) = patch.url {
            h.url = url;
        }
        if let Some(headers) = patch.headers {
            h.headers = headers;
        }
        if let Some(events) = patch.events {
            h.events = events;
        }
        if let Some(enabled) = patch.enabled {
            h.enabled = enabled;
        }
        h.updated_at = Utc::now();
        self.persist(&h).await?;
        self.cache.insert(h.id, h.clone());
        Ok(Some(h))
    }

    /// Deletes a hook.
    pub async fn delete(&self, id: Uuid) -> AppResult<bool> {
        let removed = self.cache.remove(&id).is_some();
        if removed {
            self.storage.delete(TABLE_HOOKS, &id.to_string()).await?;
        }
        Ok(removed)
    }

    async fn persist(&self, hook: &Hook) -> AppResult<()> {
        put_json(
            self.storage.as_ref(),
            TABLE_HOOKS,
            &hook.id.to_string(),
            hook,
        )
        .await
    }
}

/// A single hook definition.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Hook {
    /// Unique identifier.
    pub id: Uuid,
    /// Human-friendly name.
    pub name: String,
    /// Target URL to POST events to.
    pub url: String,
    /// Optional custom HTTP headers to send with the request.
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    /// Event names this hook subscribes to (`*` = all).
    pub events: Vec<String>,
    /// Whether this hook is enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Optional collection scope (empty = all).
    #[serde(default)]
    pub index_uids: Vec<CollectionId>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

fn default_enabled() -> bool {
    true
}

/// Patch payload for updating a hook. All fields are optional.
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct HookPatch {
    /// New name.
    pub name: Option<String>,
    /// New target URL.
    pub url: Option<String>,
    /// New headers.
    pub headers: Option<BTreeMap<String, String>>,
    /// New event list.
    pub events: Option<Vec<String>>,
    /// New enabled flag.
    pub enabled: Option<bool>,
}

/// Lists all hooks.
#[utoipa::path(
    get,
    path = "/api/v1/hooks",
    responses(
        (status = 200, description = "List of hooks", body = Vec<Hook>)
    ),
    tag = "hooks"
)]
#[get("/hooks")]
pub async fn list_hooks(state: web::Data<AppState>) -> HttpResponse {
    HttpResponse::Ok().json(state.hooks.list())
}

/// Returns a single hook.
#[utoipa::path(
    get,
    path = "/api/v1/hooks/{id}",
    params(("id" = String, Path,)),
    responses(
        (status = 200, description = "Hook", body = Hook),
        (status = 404, description = "Hook not found")
    ),
    tag = "hooks"
)]
#[get("/hooks/{id}")]
pub async fn get_hook(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let id = match Uuid::parse_str(&path) {
        Ok(u) => u,
        Err(_) => return AppError::bad_request("invalid hook id").error_response(),
    };
    match state.hooks.get(id) {
        Some(h) => HttpResponse::Ok().json(h),
        None => AppError::not_found(format!("hook {id} not found")).error_response(),
    }
}

/// Creates a new hook.
#[utoipa::path(
    post,
    path = "/api/v1/hooks",
    request_body = Hook,
    responses(
        (status = 201, description = "Hook created", body = Hook)
    ),
    tag = "hooks"
)]
#[post("/hooks")]
pub async fn create_hook(state: web::Data<AppState>, body: web::Json<Hook>) -> HttpResponse {
    let hook = body.into_inner();
    if hook.url.is_empty() {
        return AppError::bad_request("hook url is required").error_response();
    }
    match state.hooks.create(hook).await {
        Ok(h) => HttpResponse::Created().json(h),
        Err(e) => e.error_response(),
    }
}

/// Updates an existing hook.
#[utoipa::path(
    patch,
    path = "/api/v1/hooks/{id}",
    params(("id" = String, Path,)),
    request_body = HookPatch,
    responses(
        (status = 200, description = "Hook updated", body = Hook),
        (status = 404, description = "Hook not found")
    ),
    tag = "hooks"
)]
#[patch("/hooks/{id}")]
pub async fn patch_hook(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<HookPatch>,
) -> HttpResponse {
    let id = match Uuid::parse_str(&path) {
        Ok(u) => u,
        Err(_) => return AppError::bad_request("invalid hook id").error_response(),
    };
    match state.hooks.update(id, body.into_inner()).await {
        Ok(Some(h)) => HttpResponse::Ok().json(h),
        Ok(None) => AppError::not_found(format!("hook {id} not found")).error_response(),
        Err(e) => e.error_response(),
    }
}

/// Deletes a hook.
#[utoipa::path(
    delete,
    path = "/api/v1/hooks/{id}",
    params(("id" = String, Path,)),
    responses(
        (status = 204, description = "Hook deleted"),
        (status = 404, description = "Hook not found")
    ),
    tag = "hooks"
)]
#[delete("/hooks/{id}")]
pub async fn delete_hook(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let id = match Uuid::parse_str(&path) {
        Ok(u) => u,
        Err(_) => return AppError::bad_request("invalid hook id").error_response(),
    };
    match state.hooks.delete(id).await {
        Ok(true) => HttpResponse::NoContent().finish(),
        Ok(false) => AppError::not_found(format!("hook {id} not found")).error_response(),
        Err(e) => e.error_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::RedbStorage;

    #[tokio::test]
    async fn create_list_get_delete_round_trip() {
        let backend = Arc::new(RedbStorage::open_temp().unwrap());
        let svc = HookService::new(backend);
        let hook = Hook {
            id: Uuid::nil(),
            name: "test".into(),
            url: "https://example.com/hook".into(),
            headers: BTreeMap::new(),
            events: vec!["document.indexed".into()],
            enabled: true,
            index_uids: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let h = svc.create(hook).await.unwrap();
        assert!(!h.id.is_nil());
        assert_eq!(svc.list().len(), 1);
        assert!(svc.get(h.id).is_some());
        assert!(svc.delete(h.id).await.unwrap());
        assert!(svc.get(h.id).is_none());
    }

    #[tokio::test]
    async fn update_changes_fields() {
        let backend = Arc::new(RedbStorage::open_temp().unwrap());
        let svc = HookService::new(backend);
        let h = svc
            .create(Hook {
                id: Uuid::nil(),
                name: "a".into(),
                url: "https://a".into(),
                headers: BTreeMap::new(),
                events: vec!["*".into()],
                enabled: true,
                index_uids: Vec::new(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            })
            .await
            .unwrap();
        let patch = HookPatch {
            name: Some("renamed".into()),
            url: None,
            headers: None,
            events: None,
            enabled: Some(false),
        };
        let updated = svc.update(h.id, patch).await.unwrap().unwrap();
        assert_eq!(updated.name, "renamed");
        assert!(!updated.enabled);
    }

    #[tokio::test]
    async fn update_missing_returns_none() {
        let backend = Arc::new(RedbStorage::open_temp().unwrap());
        let svc = HookService::new(backend);
        let r = svc
            .update(
                Uuid::new_v4(),
                HookPatch {
                    name: None,
                    url: None,
                    headers: None,
                    events: None,
                    enabled: None,
                },
            )
            .await
            .unwrap();
        assert!(r.is_none());
    }
}
