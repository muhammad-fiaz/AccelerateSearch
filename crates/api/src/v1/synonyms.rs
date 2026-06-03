//! Synonyms REST endpoints.

use actix_web::{HttpResponse, ResponseError, delete, get, put, web};

use errors::AppError;
use models::CollectionId;

use crate::state::AppState;

/// Returns the synonyms of a collection.
#[utoipa::path(
    get,
    path = "/api/v1/collections/{uid}/settings/synonyms",
    params(("uid" = String, Path,)),
    responses(
        (status = 200, description = "Synonyms")
    ),
    tag = "synonyms"
)]
#[get("/collections/{uid}/settings/synonyms")]
pub async fn get_synonyms(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let uid = CollectionId::new(path.into_inner());
    match state.collections.get(&uid) {
        Some(_) => HttpResponse::Ok().json(serde_json::json!({})),
        None => AppError::not_found("collection not found").error_response(),
    }
}

/// Sets synonyms for a collection.
#[utoipa::path(
    put,
    path = "/api/v1/collections/{uid}/settings/synonyms",
    params(("uid" = String, Path,)),
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Synonyms updated")
    ),
    tag = "synonyms"
)]
#[put("/collections/{uid}/settings/synonyms")]
pub async fn put_synonyms(
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

/// Deletes all synonyms for a collection.
#[utoipa::path(
    delete,
    path = "/api/v1/collections/{uid}/settings/synonyms",
    params(("uid" = String, Path,)),
    responses(
        (status = 204, description = "Synonyms deleted")
    ),
    tag = "synonyms"
)]
#[delete("/collections/{uid}/settings/synonyms")]
pub async fn delete_synonyms(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let uid = CollectionId::new(path.into_inner());
    if state.collections.get(&uid).is_none() {
        return AppError::not_found("collection not found").error_response();
    }
    HttpResponse::NoContent().finish()
}
