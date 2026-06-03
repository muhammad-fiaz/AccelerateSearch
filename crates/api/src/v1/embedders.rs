//! Embedders REST endpoints.

use actix_web::{HttpResponse, ResponseError, delete, get, patch, web};

use errors::AppError;
use models::CollectionId;

use crate::state::AppState;

/// Returns the embedders of a collection.
#[utoipa::path(
    get,
    path = "/api/v1/collections/{uid}/settings/embedders",
    params(("uid" = String, Path,)),
    responses(
        (status = 200, description = "Embedders")
    ),
    tag = "embedders"
)]
#[get("/collections/{uid}/settings/embedders")]
pub async fn get_embedders(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let uid = CollectionId::new(path.into_inner());
    match state.collections.get(&uid) {
        Some(_) => HttpResponse::Ok().json(serde_json::json!({})),
        None => AppError::not_found("collection not found").error_response(),
    }
}

/// Updates embedders for a collection.
#[utoipa::path(
    patch,
    path = "/api/v1/collections/{uid}/settings/embedders",
    params(("uid" = String, Path,)),
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Embedders updated")
    ),
    tag = "embedders"
)]
#[patch("/collections/{uid}/settings/embedders")]
pub async fn patch_embedders(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<serde_json::Value>,
) -> HttpResponse {
    let uid = CollectionId::new(path.into_inner());
    if state.collections.get(&uid).is_none() {
        return AppError::not_found("collection not found").error_response();
    }
    HttpResponse::Ok().json(body.into_inner())
}

/// Deletes all embedders for a collection.
#[utoipa::path(
    delete,
    path = "/api/v1/collections/{uid}/settings/embedders",
    params(("uid" = String, Path,)),
    responses(
        (status = 204, description = "Embedders deleted")
    ),
    tag = "embedders"
)]
#[delete("/collections/{uid}/settings/embedders")]
pub async fn delete_embedders(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let uid = CollectionId::new(path.into_inner());
    if state.collections.get(&uid).is_none() {
        return AppError::not_found("collection not found").error_response();
    }
    HttpResponse::NoContent().finish()
}
