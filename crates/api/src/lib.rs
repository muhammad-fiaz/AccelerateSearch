//! REST API and OpenAPI specification for AccelerateSearch.

pub mod openapi;
pub mod state;
pub mod system;
pub mod v1;

#[cfg(test)]
mod tests;

pub use state::AppState;

use actix_web::web;

/// Configures the application's URL routing on `scope`.
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    v1::configure_v1_routes(cfg);
}

/// Configures the root (non-versioned) routes: health, version, metrics,
/// swagger UI, and openapi.json.
pub fn configure_root(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(system::health)
        .service(system::version)
        .service(system::stats)
        .service(system::metrics)
        .service(system::instance_id);
}
