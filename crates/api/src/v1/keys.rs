//! Keys REST endpoints.

use actix_web::{HttpResponse, ResponseError, delete, get, patch, post, web};
use chrono::{DateTime, Utc};
use models::{ApiKeyId, CollectionId, Permission};
use serde::{Deserialize, Serialize};

use auth::AuthService;
use errors::{AppError, AppResult};
use validation::validate_api_key_name;

use crate::state::AppState;

/// Create-key request body.
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct CreateKeyRequest {
    /// Key name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Permissions.
    pub actions: Vec<Permission>,
    /// Scoped collections (empty/None = all).
    pub indexes: Option<Vec<String>>,
    /// Expiry timestamp.
    pub expires_at: Option<DateTime<Utc>>,
}

/// Create-key response.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct CreateKeyResponse {
    /// Key metadata.
    #[serde(flatten)]
    pub key: models::ApiKey,
    /// Plaintext key (only shown once at creation).
    pub key_plaintext: String,
}

/// Lists API keys.
#[utoipa::path(
    get,
    path = "/api/v1/keys",
    responses(
        (status = 200, description = "List of keys", body = KeysResponse)
    ),
    tag = "keys"
)]
#[get("/keys")]
pub async fn list_keys(state: web::Data<AppState>) -> HttpResponse {
    match state.auth.list_keys().await {
        Ok(keys) => HttpResponse::Ok().json(KeysResponse { results: keys }),
        Err(e) => e.error_response(),
    }
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct KeysResponse {
    /// Returned keys.
    pub results: Vec<models::ApiKey>,
}

/// Creates an API key.
#[utoipa::path(
    post,
    path = "/api/v1/keys",
    request_body = CreateKeyRequest,
    responses(
        (status = 201, description = "Key created", body = CreateKeyResponse)
    ),
    tag = "keys"
)]
#[post("/keys")]
pub async fn create_key(
    state: web::Data<AppState>,
    body: web::Json<CreateKeyRequest>,
) -> HttpResponse {
    if let Err(e) = validate_api_key_name(&body.name) {
        return e.error_response();
    }
    let indexes = body
        .indexes
        .clone()
        .map(|v| v.into_iter().map(CollectionId::new).collect());
    match state
        .auth
        .create_key(
            &body.name,
            body.description.clone(),
            body.actions.clone(),
            indexes,
            body.expires_at,
        )
        .await
    {
        Ok((key, plaintext)) => HttpResponse::Created().json(CreateKeyResponse {
            key,
            key_plaintext: plaintext,
        }),
        Err(e) => e.error_response(),
    }
}

/// Returns a single API key.
#[utoipa::path(
    get,
    path = "/api/v1/keys/{keyOrUid}",
    params(("keyOrUid" = String, Path,)),
    responses(
        (status = 200, description = "Key", body = models::ApiKey)
    ),
    tag = "keys"
)]
#[get("/keys/{key_or_uid}")]
pub async fn get_key(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let raw = path.into_inner();
    if let Ok(uuid) = uuid::Uuid::parse_str(&raw) {
        match state.auth.get_key(ApiKeyId::from_uuid(uuid)).await {
            Ok(Some(k)) => return HttpResponse::Ok().json(k),
            Ok(None) => return AppError::not_found("key not found").error_response(),
            Err(e) => return e.error_response(),
        }
    }
    AppError::not_found("key not found").error_response()
}

/// Patch-key request.
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct PatchKeyRequest {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub actions: Option<Vec<Permission>>,
    pub indexes: Option<Option<Vec<String>>>,
    pub expires_at: Option<Option<DateTime<Utc>>>,
}

/// Updates an API key.
#[utoipa::path(
    patch,
    path = "/api/v1/keys/{keyOrUid}",
    params(("keyOrUid" = String, Path,)),
    request_body = PatchKeyRequest,
    responses(
        (status = 200, description = "Updated key", body = models::ApiKey)
    ),
    tag = "keys"
)]
#[patch("/keys/{key_or_uid}")]
pub async fn patch_key(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<PatchKeyRequest>,
) -> HttpResponse {
    let raw = path.into_inner();
    let uid = match uuid::Uuid::parse_str(&raw) {
        Ok(u) => ApiKeyId::from_uuid(u),
        Err(_) => return AppError::bad_request("invalid key uid").error_response(),
    };
    let indexes = body
        .indexes
        .clone()
        .map(|opt| opt.map(|v| v.into_iter().map(CollectionId::new).collect()));
    match state
        .auth
        .update_key(
            uid,
            body.name.clone(),
            body.description.clone(),
            body.actions.clone(),
            indexes,
            body.expires_at,
        )
        .await
    {
        Ok(k) => HttpResponse::Ok().json(k),
        Err(e) => e.error_response(),
    }
}

/// Deletes an API key.
#[utoipa::path(
    delete,
    path = "/api/v1/keys/{keyOrUid}",
    params(("keyOrUid" = String, Path,)),
    responses(
        (status = 204, description = "Key deleted")
    ),
    tag = "keys"
)]
#[delete("/keys/{key_or_uid}")]
pub async fn delete_key(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let raw = path.into_inner();
    let uid = match uuid::Uuid::parse_str(&raw) {
        Ok(u) => ApiKeyId::from_uuid(u),
        Err(_) => return AppError::bad_request("invalid key uid").error_response(),
    };
    match state.auth.delete_key(uid).await {
        Ok(true) => HttpResponse::NoContent().finish(),
        Ok(false) => AppError::not_found("key not found").error_response(),
        Err(e) => e.error_response(),
    }
}

#[allow(dead_code)]
fn _suppress_unused(_a: &AppState, _b: &AuthService) -> AppResult<()> {
    Ok(())
}
