//! Snapshots REST endpoints.

use actix_web::{HttpResponse, ResponseError, delete, get, post, web};

use errors::AppError;

use crate::state::AppState;

/// Creates a snapshot of the live database.
#[utoipa::path(
    post,
    path = "/api/v1/snapshots",
    responses(
        (status = 202, description = "Snapshot created", body = models::SnapshotMeta),
        (status = 500, description = "Internal error", body = errors::ErrorBody)
    ),
    tag = "snapshots"
)]
#[post("/snapshots")]
pub async fn create_snapshot(state: web::Data<AppState>) -> HttpResponse {
    let data_dir = state.config.data.dir.clone();
    let db_file = data_dir.join("accelerate.redb");
    match state.snapshots.create(&db_file).await {
        Ok(meta) => HttpResponse::Accepted().json(meta),
        Err(e) => e.error_response(),
    }
}

/// Lists all known snapshots, newest first.
#[utoipa::path(
    get,
    path = "/api/v1/snapshots",
    responses(
        (status = 200, description = "List of snapshots", body = Vec<snapshots::SnapshotSummary>)
    ),
    tag = "snapshots"
)]
#[get("/snapshots")]
pub async fn list_snapshots(state: web::Data<AppState>) -> HttpResponse {
    match state.snapshots.list().await {
        Ok(list) => {
            let body: Vec<snapshots::SnapshotSummary> = list
                .into_iter()
                .map(snapshots::SnapshotSummary::from)
                .collect();
            HttpResponse::Ok().json(body)
        }
        Err(e) => e.error_response(),
    }
}

/// Returns a single snapshot's metadata by name.
#[utoipa::path(
    get,
    path = "/api/v1/snapshots/{name}",
    params(("name" = String, Path,)),
    responses(
        (status = 200, description = "Snapshot metadata", body = models::SnapshotMeta),
        (status = 404, description = "Snapshot not found", body = errors::ErrorBody)
    ),
    tag = "snapshots"
)]
#[get("/snapshots/{name}")]
pub async fn get_snapshot(state: web::Data<AppState>, name: web::Path<String>) -> HttpResponse {
    let name = name.into_inner();
    match state.snapshots.get(&name).await {
        Ok(Some(meta)) => HttpResponse::Ok().json(meta),
        Ok(None) => AppError::not_found(format!("snapshot '{name}' not found")).error_response(),
        Err(e) => e.error_response(),
    }
}

/// Deletes a snapshot by name.
#[utoipa::path(
    delete,
    path = "/api/v1/snapshots/{name}",
    params(("name" = String, Path,)),
    responses(
        (status = 204, description = "Snapshot deleted"),
        (status = 404, description = "Snapshot not found", body = errors::ErrorBody)
    ),
    tag = "snapshots"
)]
#[delete("/snapshots/{name}")]
pub async fn delete_snapshot(state: web::Data<AppState>, name: web::Path<String>) -> HttpResponse {
    let name = name.into_inner();
    match state.snapshots.delete(&name).await {
        Ok(true) => HttpResponse::NoContent().finish(),
        Ok(false) => AppError::not_found(format!("snapshot '{name}' not found")).error_response(),
        Err(e) => e.error_response(),
    }
}

/// Restores the database from a snapshot. The server is expected to be
/// stopped before this endpoint is called; restoring replaces the live
/// `accelerate.redb` file in the data directory with the contents of the
/// archive. The handler returns 204 on success.
#[utoipa::path(
    post,
    path = "/api/v1/snapshots/{name}/restore",
    params(("name" = String, Path,)),
    responses(
        (status = 204, description = "Snapshot restored"),
        (status = 404, description = "Snapshot not found", body = errors::ErrorBody),
        (status = 500, description = "Restore failed", body = errors::ErrorBody)
    ),
    tag = "snapshots"
)]
#[post("/snapshots/{name}/restore")]
pub async fn restore_snapshot(state: web::Data<AppState>, name: web::Path<String>) -> HttpResponse {
    let name = name.into_inner();
    let data_dir = state.config.data.dir.clone();
    let target = data_dir.join("accelerate.redb");
    let meta = match state.snapshots.get(&name).await {
        Ok(Some(m)) => m,
        Ok(None) => {
            return AppError::not_found(format!("snapshot '{name}' not found")).error_response();
        }
        Err(e) => return e.error_response(),
    };
    let archive = std::path::PathBuf::from(&meta.path);
    match state.snapshots.restore(&archive, &target).await {
        Ok(()) => HttpResponse::NoContent().finish(),
        Err(e) => e.error_response(),
    }
}
