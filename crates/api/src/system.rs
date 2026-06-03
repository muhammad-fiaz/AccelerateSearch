//! System endpoints: health, version, stats, metrics.

use actix_web::{HttpResponse, get, web};
use serde::Serialize;
use utoipa::ToSchema;

use models::{GlobalStats, Health, VersionInfo};
use utils::new_uuid_v4;

use crate::state::AppState;

/// Returns the service health.
#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Service is healthy", body = Health)
    ),
    tag = "system"
)]
#[get("/health")]
pub async fn health() -> HttpResponse {
    HttpResponse::Ok().json(Health {
        status: "available".into(),
    })
}

/// Returns the service version.
#[utoipa::path(
    get,
    path = "/version",
    responses(
        (status = 200, description = "Version information", body = VersionInfo)
    ),
    tag = "system"
)]
#[get("/version")]
pub async fn version() -> HttpResponse {
    HttpResponse::Ok().json(VersionInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        commit_sha: option_env!("ACCELERATE_GIT_SHA").map(str::to_string),
        commit_date: option_env!("ACCELERATE_GIT_DATE").map(str::to_string),
    })
}

/// Returns global statistics.
#[utoipa::path(
    get,
    path = "/stats",
    responses(
        (status = 200, description = "Global stats", body = GlobalStats)
    ),
    tag = "system"
)]
#[get("/stats")]
pub async fn stats(state: web::Data<AppState>) -> HttpResponse {
    let counts = state.collections.document_counts();
    let total: u64 = counts.values().sum();
    let body = GlobalStats {
        number_of_collections: counts.len() as u64,
        number_of_documents: total,
        is_indexing: false,
    };
    HttpResponse::Ok().json(body)
}

/// Returns Prometheus metrics (gated by config).
#[utoipa::path(
    get,
    path = "/metrics",
    responses(
        (status = 200, description = "Prometheus metrics", content_type = "text/plain")
    ),
    tag = "system"
)]
#[get("/metrics")]
pub async fn metrics(state: web::Data<AppState>) -> HttpResponse {
    if !state.config.metrics.enabled {
        return HttpResponse::NotFound().finish();
    }
    let body = accelerate_metrics::gather();
    HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4")
        .body(body)
}

/// Returns an opaque server-assigned instance id.
#[derive(Debug, Serialize, ToSchema)]
pub struct InstanceId {
    /// UUID v4 identifying this process instance.
    pub id: String,
}

/// Returns a fresh instance id (useful for diagnostics).
#[utoipa::path(
    get,
    path = "/instance-id",
    responses(
        (status = 200, description = "Server-generated instance id", body = InstanceId)
    ),
    tag = "system"
)]
#[get("/instance-id")]
pub async fn instance_id() -> HttpResponse {
    HttpResponse::Ok().json(InstanceId {
        id: new_uuid_v4().to_string(),
    })
}
