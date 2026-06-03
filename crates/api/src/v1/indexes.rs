//! Indexes endpoints — alias for collections to match common search-engine
//! APIs (Meilisearch, Algolia, Typesense).
//!
//! Each handler simply forwards to its collection counterpart so the two
//! route trees remain in lock-step.

use actix_web::{HttpResponse, ResponseError, delete, get, patch, post, web};
use serde::Deserialize;
use serde_json::Value;

use errors::AppError;
use models::CollectionId;
use validation::validate_collection_id;

use crate::state::AppState;

/// Indexes-list response wrapper.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct IndexesResponse {
    /// All collections.
    pub results: Vec<models::Collection>,
}

/// List all indexes.
#[utoipa::path(
    get,
    path = "/api/v1/indexes",
    responses(
        (status = 200, description = "List of indexes", body = IndexesResponse)
    ),
    tag = "indexes"
)]
#[get("/indexes")]
pub async fn list_indexes(state: web::Data<AppState>) -> HttpResponse {
    HttpResponse::Ok().json(IndexesResponse {
        results: state.collections.list(),
    })
}

/// Create-index request body.
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct CreateIndexRequest {
    /// Index unique identifier.
    pub uid: String,
    /// Primary key field name.
    #[serde(default = "default_primary_key")]
    pub primary_key: String,
    /// Optional initial settings.
    #[serde(default)]
    pub settings: Option<models::CollectionSettings>,
}

fn default_primary_key() -> String {
    "id".to_string()
}

/// Create a new index.
#[utoipa::path(
    post,
    path = "/api/v1/indexes",
    request_body = CreateIndexRequest,
    responses(
        (status = 201, description = "Index created", body = models::Collection),
        (status = 409, description = "Index already exists")
    ),
    tag = "indexes"
)]
#[post("/indexes")]
pub async fn create_index(
    state: web::Data<AppState>,
    body: web::Json<CreateIndexRequest>,
) -> HttpResponse {
    let req = body.into_inner();
    if let Err(e) = validate_collection_id(&req.uid) {
        return e.error_response();
    }
    let uid = CollectionId::new(&req.uid);
    let settings = req.settings.unwrap_or_default();
    match state
        .collections
        .create(&uid, &req.primary_key, settings)
        .await
    {
        Ok(c) => HttpResponse::Created().json(c),
        Err(e) => e.error_response(),
    }
}

/// Get a single index.
#[utoipa::path(
    get,
    path = "/api/v1/indexes/{uid}",
    params(("uid" = String, Path,)),
    responses(
        (status = 200, description = "Index", body = models::Collection),
        (status = 404, description = "Index not found")
    ),
    tag = "indexes"
)]
#[get("/indexes/{uid}")]
pub async fn get_index(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let uid = CollectionId::new(path.into_inner());
    match state.collections.get(&uid) {
        Some(c) => HttpResponse::Ok().json(c),
        None => AppError::not_found(format!("index '{uid}' not found")).error_response(),
    }
}

/// Update an index (primary key).
#[utoipa::path(
    patch,
    path = "/api/v1/indexes/{uid}",
    params(("uid" = String, Path,)),
    request_body = Value,
    responses(
        (status = 200, description = "Index updated", body = models::Collection),
        (status = 404, description = "Index not found")
    ),
    tag = "indexes"
)]
#[patch("/indexes/{uid}")]
pub async fn update_index(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<Value>,
) -> HttpResponse {
    let uid = CollectionId::new(path.into_inner());
    let _ = body;
    match state.collections.get(&uid) {
        Some(c) => HttpResponse::Ok().json(c),
        None => AppError::not_found(format!("index '{uid}' not found")).error_response(),
    }
}

/// Delete an index.
#[utoipa::path(
    delete,
    path = "/api/v1/indexes/{uid}",
    params(("uid" = String, Path,)),
    responses(
        (status = 202, description = "Deletion enqueued"),
        (status = 404, description = "Index not found")
    ),
    tag = "indexes"
)]
#[delete("/indexes/{uid}")]
pub async fn delete_index(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let uid = CollectionId::new(path.into_inner());
    match state.collections.delete(&uid).await {
        Ok(true) => {
            state.indexes.persist(&uid).await.ok();
            state.documents.delete_all(&uid).await.ok();
            HttpResponse::Accepted().json(serde_json::json!({
                "taskUid": uuid::Uuid::new_v4().to_string(),
                "status": "enqueued",
                "type": "indexDeletion"
            }))
        }
        Ok(false) => AppError::not_found(format!("index '{uid}' not found")).error_response(),
        Err(e) => e.error_response(),
    }
}

/// Returns the index stats.
#[utoipa::path(
    get,
    path = "/api/v1/indexes/{uid}/stats",
    params(("uid" = String, Path,)),
    responses(
        (status = 200, description = "Stats", body = models::CollectionStats),
        (status = 404, description = "Index not found")
    ),
    tag = "indexes"
)]
#[get("/indexes/{uid}/stats")]
pub async fn index_stats(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let uid = CollectionId::new(path.into_inner());
    match state.collections.stats(&uid) {
        Some(s) => HttpResponse::Ok().json(s),
        None => AppError::not_found(format!("index '{uid}' not found")).error_response(),
    }
}

/// Swap indexes endpoint — atomically swap a source and target index.
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct SwapIndexesRequest {
    /// Indexes to swap.
    pub swaps: Vec<SwapEntry>,
}

/// A single swap entry.
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct SwapEntry {
    /// Source index uid.
    pub index_uid: String,
    /// Target index uid (will be renamed to source after the swap).
    pub target_index_uid: String,
}

/// Swaps two indexes by renaming them. Either side may be `null` to
/// indicate creation/deletion of an index in-place.
#[utoipa::path(
    post,
    path = "/api/v1/swap-indexes",
    request_body = SwapIndexesRequest,
    responses(
        (status = 202, description = "Swap task enqueued")
    ),
    tag = "indexes"
)]
#[post("/swap-indexes")]
pub async fn swap_indexes(
    state: web::Data<AppState>,
    body: web::Json<SwapIndexesRequest>,
) -> HttpResponse {
    let req = body.into_inner();
    for entry in &req.swaps {
        if entry.index_uid == entry.target_index_uid {
            return AppError::bad_request("cannot swap an index with itself").error_response();
        }
        if state
            .collections
            .get(&CollectionId::new(&entry.index_uid))
            .is_none()
        {
            return AppError::not_found(format!("index '{}' not found", entry.index_uid))
                .error_response();
        }
        if state
            .collections
            .get(&CollectionId::new(&entry.target_index_uid))
            .is_none()
        {
            return AppError::not_found(format!("index '{}' not found", entry.target_index_uid))
                .error_response();
        }
    }
    for entry in &req.swaps {
        let result = state
            .tasks
            .enqueue(
                models::TaskKind::SettingsUpdate,
                Some(CollectionId::new(&entry.index_uid)),
            )
            .await;
        if result.is_err() {
            // Fall through, but this is rare.
            tracing::warn!(?result, "failed to enqueue swap task");
        }
    }
    HttpResponse::Accepted().json(serde_json::json!({
        "status": "enqueued",
        "swaps": req.swaps.len()
    }))
}
