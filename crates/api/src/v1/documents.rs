//! Documents REST endpoints.

use actix_web::{HttpResponse, ResponseError, delete, get, post, put, web};
use models::{CollectionId, DocumentId};
use serde::Deserialize;
use serde_json::Value;

use errors::AppError;
use validation::validate_pagination;

use crate::state::AppState;

/// Request body for add/replace and add/update endpoints.
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct DocumentsBody {
    /// Documents to ingest.
    pub documents: Vec<Value>,
}

/// Batch delete request.
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct BatchDeleteRequest {
    /// Document ids to delete.
    pub ids: Vec<String>,
}

/// Adds or replaces documents.
#[utoipa::path(
    post,
    path = "/api/v1/collections/{uid}/documents",
    params(("uid" = String, Path,)),
    request_body = DocumentsBody,
    responses(
        (status = 202, description = "Task enqueued"),
        (status = 400, description = "Invalid body"),
        (status = 404, description = "Collection not found")
    ),
    tag = "documents"
)]
#[post("/collections/{uid}/documents")]
pub async fn add_documents(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<DocumentsBody>,
) -> HttpResponse {
    let uid = CollectionId::new(path.into_inner());
    if state.collections.get(&uid).is_none() {
        return AppError::not_found(format!("collection '{uid}' not found")).error_response();
    }
    let docs: Vec<models::Document> = body
        .into_inner()
        .documents
        .into_iter()
        .filter_map(|v| match v {
            Value::Object(m) => Some(m.into_iter().collect()),
            _ => None,
        })
        .collect();
    match state.documents.add_or_replace(&uid, docs).await {
        Ok(n) => HttpResponse::Accepted().json(serde_json::json!({
            "indexed": n,
            "status": "succeeded"
        })),
        Err(e) => e.error_response(),
    }
}

/// Adds or updates documents.
#[utoipa::path(
    put,
    path = "/api/v1/collections/{uid}/documents",
    params(("uid" = String, Path,)),
    request_body = DocumentsBody,
    responses(
        (status = 202, description = "Task enqueued"),
        (status = 404, description = "Collection not found")
    ),
    tag = "documents"
)]
#[put("/collections/{uid}/documents")]
pub async fn update_documents(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<DocumentsBody>,
) -> HttpResponse {
    let uid = CollectionId::new(path.into_inner());
    if state.collections.get(&uid).is_none() {
        return AppError::not_found(format!("collection '{uid}' not found")).error_response();
    }
    let docs: Vec<models::Document> = body
        .into_inner()
        .documents
        .into_iter()
        .filter_map(|v| match v {
            Value::Object(m) => Some(m.into_iter().collect()),
            _ => None,
        })
        .collect();
    match state.documents.add_or_update(&uid, docs).await {
        Ok(n) => HttpResponse::Accepted().json(serde_json::json!({ "indexed": n })),
        Err(e) => e.error_response(),
    }
}

/// Lists documents (paginated).
#[utoipa::path(
    get,
    path = "/api/v1/collections/{uid}/documents",
    params(
        ("uid" = String, Path,),
        ("offset" = Option<usize>, Query,),
        ("limit" = Option<usize>, Query,)
    ),
    responses(
        (status = 200, description = "Paginated documents", body = Vec<models::DocumentDto>),
        (status = 404, description = "Collection not found")
    ),
    tag = "documents"
)]
#[get("/collections/{uid}/documents")]
pub async fn list_documents(
    state: web::Data<AppState>,
    path: web::Path<String>,
    query: web::Query<PaginationQuery>,
) -> HttpResponse {
    let uid = CollectionId::new(path.into_inner());
    if state.collections.get(&uid).is_none() {
        return AppError::not_found(format!("collection '{uid}' not found")).error_response();
    }
    if let Err(e) = validate_pagination(
        query.offset,
        query.limit,
        state.config.search.pagination_max_total_hits,
    ) {
        return e.error_response();
    }
    match state.documents.list(&uid, query.offset, query.limit).await {
        Ok(docs) => {
            let body: Vec<models::DocumentDto> =
                docs.into_iter().map(models::DocumentDto).collect();
            HttpResponse::Ok().json(serde_json::json!({
                "results": body,
                "offset": query.offset,
                "limit": query.limit,
                "total": body.len()
            }))
        }
        Err(e) => e.error_response(),
    }
}

/// Pagination query.
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct PaginationQuery {
    /// Page offset.
    #[serde(default)]
    pub offset: usize,
    /// Page limit.
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    20
}

/// Returns a single document.
#[utoipa::path(
    get,
    path = "/api/v1/collections/{uid}/documents/{id}",
    params(("uid" = String, Path,), ("id" = String, Path,)),
    responses(
        (status = 200, description = "Document", body = models::DocumentDto),
        (status = 404, description = "Not found")
    ),
    tag = "documents"
)]
#[get("/collections/{uid}/documents/{id}")]
pub async fn get_document(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> HttpResponse {
    let (uid_str, id) = path.into_inner();
    let uid = CollectionId::new(uid_str);
    match state
        .documents
        .get(&uid, &DocumentId::new(id.clone()))
        .await
    {
        Ok(Some(d)) => HttpResponse::Ok().json(d),
        Ok(None) => AppError::not_found(format!("document '{id}' not found")).error_response(),
        Err(e) => e.error_response(),
    }
}

/// Deletes a single document.
#[utoipa::path(
    delete,
    path = "/api/v1/collections/{uid}/documents/{id}",
    params(("uid" = String, Path,), ("id" = String, Path,)),
    responses(
        (status = 202, description = "Deleted"),
        (status = 404, description = "Not found")
    ),
    tag = "documents"
)]
#[delete("/collections/{uid}/documents/{id}")]
pub async fn delete_document(
    state: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> HttpResponse {
    let (uid_str, id) = path.into_inner();
    let uid = CollectionId::new(uid_str);
    match state.documents.delete(&uid, &DocumentId::new(id)).await {
        Ok(_) => HttpResponse::Accepted().json(serde_json::json!({ "status": "succeeded" })),
        Err(e) => e.error_response(),
    }
}

/// Deletes all documents in a collection.
#[utoipa::path(
    delete,
    path = "/api/v1/collections/{uid}/documents",
    params(("uid" = String, Path,)),
    responses(
        (status = 202, description = "Deletion enqueued")
    ),
    tag = "documents"
)]
#[delete("/collections/{uid}/documents")]
pub async fn delete_all_documents(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let uid = CollectionId::new(path.into_inner());
    match state.documents.delete_all(&uid).await {
        Ok(_) => HttpResponse::Accepted().json(serde_json::json!({ "status": "succeeded" })),
        Err(e) => e.error_response(),
    }
}

/// Batch delete by IDs.
#[utoipa::path(
    post,
    path = "/api/v1/collections/{uid}/documents/delete-batch",
    params(("uid" = String, Path,)),
    request_body = BatchDeleteRequest,
    responses(
        (status = 202, description = "Deletion enqueued")
    ),
    tag = "documents"
)]
#[post("/collections/{uid}/documents/delete-batch")]
pub async fn delete_batch(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<BatchDeleteRequest>,
) -> HttpResponse {
    let uid = CollectionId::new(path.into_inner());
    let ids: Vec<DocumentId> = body
        .into_inner()
        .ids
        .into_iter()
        .map(DocumentId::new)
        .collect();
    match state.documents.delete_many(&uid, &ids).await {
        Ok(n) => HttpResponse::Accepted().json(serde_json::json!({ "deleted": n })),
        Err(e) => e.error_response(),
    }
}
