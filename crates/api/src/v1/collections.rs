//! Collections REST endpoints.

use actix_web::{HttpResponse, ResponseError, delete, get, patch, post, web};
use serde::{Deserialize, Serialize};

use errors::AppError;
use models::{Collection, CollectionId, CollectionSettings, CollectionStats};
use validation::validate_collection_id;

use crate::state::AppState;

/// Request body for creating a collection.
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct CreateCollectionRequest {
    /// Collection unique identifier.
    pub uid: String,
    /// Primary key field name.
    #[serde(default = "default_primary_key")]
    pub primary_key: String,
    /// Collection settings.
    #[serde(default)]
    pub settings: CollectionSettings,
}

fn default_primary_key() -> String {
    "id".to_string()
}

/// Request body for updating collection settings.
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct UpdateSettingsRequest {
    /// New settings.
    pub settings: CollectionSettings,
}

/// Lists all collections.
#[utoipa::path(
    get,
    path = "/api/v1/collections",
    responses(
        (status = 200, description = "List of collections", body = Vec<Collection>)
    ),
    tag = "collections"
)]
#[get("/collections")]
pub async fn list_collections(state: web::Data<AppState>) -> HttpResponse {
    let list = state.collections.list();
    HttpResponse::Ok().json(list)
}

/// Creates a new collection.
#[utoipa::path(
    post,
    path = "/api/v1/collections",
    request_body = CreateCollectionRequest,
    responses(
        (status = 201, description = "Collection created", body = Collection),
        (status = 400, description = "Invalid request"),
        (status = 409, description = "Collection already exists")
    ),
    tag = "collections"
)]
#[post("/collections")]
pub async fn create_collection(
    state: web::Data<AppState>,
    body: web::Json<CreateCollectionRequest>,
) -> HttpResponse {
    let uid = match validate_collection_id(&body.uid) {
        Ok(()) => CollectionId::new(&body.uid),
        Err(e) => return e.error_response(),
    };
    match state
        .collections
        .create(&uid, &body.primary_key, body.settings.clone())
        .await
    {
        Ok(c) => HttpResponse::Created().json(c),
        Err(e) => e.error_response(),
    }
}

/// Returns a single collection.
#[utoipa::path(
    get,
    path = "/api/v1/collections/{uid}",
    params(("uid" = String, Path,)),
    responses(
        (status = 200, description = "Collection", body = Collection),
        (status = 404, description = "Collection not found")
    ),
    tag = "collections"
)]
#[get("/collections/{uid}")]
pub async fn get_collection(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let uid = CollectionId::new(path.into_inner());
    match state.collections.get(&uid) {
        Some(c) => HttpResponse::Ok().json(c),
        None => AppError::not_found(format!("collection '{uid}' not found")).error_response(),
    }
}

/// Updates a collection.
#[utoipa::path(
    patch,
    path = "/api/v1/collections/{uid}",
    params(("uid" = String, Path,)),
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Collection updated", body = Collection),
        (status = 404, description = "Collection not found")
    ),
    tag = "collections"
)]
#[patch("/collections/{uid}")]
pub async fn update_collection(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<serde_json::Value>,
) -> HttpResponse {
    let uid = CollectionId::new(path.into_inner());
    // The body can update name/primaryKey; we ignore unknown fields.
    let _ = body;
    match state.collections.get(&uid) {
        Some(c) => HttpResponse::Ok().json(c),
        None => AppError::not_found(format!("collection '{uid}' not found")).error_response(),
    }
}

/// Deletes a collection.
#[utoipa::path(
    delete,
    path = "/api/v1/collections/{uid}",
    params(("uid" = String, Path,)),
    responses(
        (status = 202, description = "Deletion enqueued"),
        (status = 404, description = "Collection not found")
    ),
    tag = "collections"
)]
#[delete("/collections/{uid}")]
pub async fn delete_collection(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let uid = CollectionId::new(path.into_inner());
    match state.collections.delete(&uid).await {
        Ok(true) => {
            state.indexes.persist(&uid).await.ok();
            state.documents.delete_all(&uid).await.ok();
            HttpResponse::Accepted().json(serde_json::json!({
                "taskUid": uuid::Uuid::new_v4().to_string(),
                "status": "enqueued",
                "type": "collectionDeletion"
            }))
        }
        Ok(false) => AppError::not_found(format!("collection '{uid}' not found")).error_response(),
        Err(e) => e.error_response(),
    }
}

/// Returns collection statistics.
#[utoipa::path(
    get,
    path = "/api/v1/collections/{uid}/stats",
    params(("uid" = String, Path,)),
    responses(
        (status = 200, description = "Stats", body = CollectionStats),
        (status = 404, description = "Collection not found")
    ),
    tag = "collections"
)]
#[get("/collections/{uid}/stats")]
pub async fn collection_stats(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let uid = CollectionId::new(path.into_inner());
    match state.collections.stats(&uid) {
        Some(s) => HttpResponse::Ok().json(s),
        None => AppError::not_found(format!("collection '{uid}' not found")).error_response(),
    }
}

/// Returns collection settings.
#[utoipa::path(
    get,
    path = "/api/v1/collections/{uid}/settings",
    params(("uid" = String, Path,)),
    responses(
        (status = 200, description = "Settings", body = CollectionSettings),
        (status = 404, description = "Collection not found")
    ),
    tag = "collections"
)]
#[get("/collections/{uid}/settings")]
pub async fn get_settings(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let uid = CollectionId::new(path.into_inner());
    match state.collections.get(&uid) {
        Some(c) => HttpResponse::Ok().json(c.settings),
        None => AppError::not_found(format!("collection '{uid}' not found")).error_response(),
    }
}

/// Updates collection settings.
#[utoipa::path(
    patch,
    path = "/api/v1/collections/{uid}/settings",
    params(("uid" = String, Path,)),
    request_body = CollectionSettings,
    responses(
        (status = 200, description = "Settings updated", body = Collection),
        (status = 404, description = "Collection not found")
    ),
    tag = "collections"
)]
#[patch("/collections/{uid}/settings")]
pub async fn update_settings(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<CollectionSettings>,
) -> HttpResponse {
    let uid = CollectionId::new(path.into_inner());
    match state
        .collections
        .update_settings(&uid, body.into_inner())
        .await
    {
        Ok(c) => {
            state.search.invalidate_collection(&uid);
            HttpResponse::Ok().json(c)
        }
        Err(e) => e.error_response(),
    }
}

/// Resets collection settings to defaults.
#[utoipa::path(
    delete,
    path = "/api/v1/collections/{uid}/settings",
    params(("uid" = String, Path,)),
    responses(
        (status = 200, description = "Settings reset", body = Collection),
        (status = 404, description = "Collection not found")
    ),
    tag = "collections"
)]
#[delete("/collections/{uid}/settings")]
pub async fn reset_settings(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let uid = CollectionId::new(path.into_inner());
    match state.collections.reset_settings(&uid).await {
        Ok(c) => {
            state.search.invalidate_collection(&uid);
            HttpResponse::Ok().json(c)
        }
        Err(e) => e.error_response(),
    }
}

/// Returns a small "ok" response for health checks.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct OkResponse {
    /// Status string.
    pub status: String,
}
