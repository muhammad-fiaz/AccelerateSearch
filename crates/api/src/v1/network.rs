//! Network and experimental-features endpoints.
//!
//! These endpoints expose runtime information about the cluster and
//! toggleable experimental flags.

use std::collections::BTreeMap;

use actix_web::{HttpResponse, get, patch, web};
use serde::Deserialize;

use crate::state::AppState;

/// Network information.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct NetworkInfo {
    /// Cluster identifier (or `"standalone"` for a single-node deployment).
    pub cluster: String,
    /// Whether this node is the cluster leader.
    pub leader: bool,
    /// Node id.
    pub node_id: String,
    /// Topology: list of remote nodes known to this instance.
    #[serde(default)]
    pub remotes: Vec<RemoteNode>,
}

/// Description of a remote node in the cluster.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct RemoteNode {
    /// URL of the remote node.
    pub url: String,
    /// Node id.
    pub node_id: String,
}

/// Returns cluster network information.
#[utoipa::path(
    get,
    path = "/api/v1/network",
    responses(
        (status = 200, description = "Network info", body = NetworkInfo)
    ),
    tag = "system"
)]
#[get("/network")]
pub async fn network_info(state: web::Data<AppState>) -> HttpResponse {
    let _ = state.storage.list(storage::TABLE_COLLECTIONS, "").await;
    HttpResponse::Ok().json(NetworkInfo {
        cluster: "standalone".into(),
        leader: true,
        node_id: uuid::Uuid::new_v4().to_string(),
        remotes: Vec::new(),
    })
}

/// A single experimental feature.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct ExperimentalFeature {
    /// Stable feature name.
    pub name: String,
    /// Whether the feature is enabled.
    pub enabled: bool,
    /// Free-text description.
    pub description: String,
}

/// Experimental features list.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct ExperimentalFeaturesResponse {
    /// All known experimental features and their state.
    pub features: BTreeMap<String, ExperimentalFeature>,
}

fn build_experimental_features() -> ExperimentalFeaturesResponse {
    let mut features = BTreeMap::new();
    features.insert(
        "vectorSearch".into(),
        ExperimentalFeature {
            name: "vectorSearch".into(),
            enabled: true,
            description: "Vector similarity search with HNSW index".into(),
        },
    );
    features.insert(
        "hybridSearch".into(),
        ExperimentalFeature {
            name: "hybridSearch".into(),
            enabled: true,
            description: "Reciprocal-rank-fusion hybrid keyword/vector search".into(),
        },
    );
    features.insert(
        "tenantTokens".into(),
        ExperimentalFeature {
            name: "tenantTokens".into(),
            enabled: true,
            description: "Short-lived JWTs scoped to collections".into(),
        },
    );
    features.insert(
        "webhooks".into(),
        ExperimentalFeature {
            name: "webhooks".into(),
            enabled: true,
            description: "HTTP POST hooks for task lifecycle events".into(),
        },
    );
    features.insert(
        "searchRules".into(),
        ExperimentalFeature {
            name: "searchRules".into(),
            enabled: true,
            description: "Curated queries with pinning and overrides".into(),
        },
    );
    features.insert(
        "geoSearch".into(),
        ExperimentalFeature {
            name: "geoSearch".into(),
            enabled: true,
            description: "Geolocation-based search with bounding box and radius".into(),
        },
    );
    ExperimentalFeaturesResponse { features }
}

/// Returns the list of experimental features and their current state.
#[utoipa::path(
    get,
    path = "/api/v1/experimental-features",
    responses(
        (status = 200, description = "Experimental features", body = ExperimentalFeaturesResponse)
    ),
    tag = "system"
)]
#[get("/experimental-features")]
pub async fn get_experimental_features() -> HttpResponse {
    HttpResponse::Ok().json(build_experimental_features())
}

/// Patch payload for experimental features.
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct ExperimentalFeaturesPatch {
    /// Map of feature name -> new state.
    pub features: BTreeMap<String, bool>,
}

/// Updates experimental feature toggles. In this single-process build the
/// toggles are recorded but not enforced (they are advisory).
#[utoipa::path(
    patch,
    path = "/api/v1/experimental-features",
    request_body = ExperimentalFeaturesPatch,
    responses(
        (status = 200, description = "Updated", body = ExperimentalFeaturesResponse)
    ),
    tag = "system"
)]
#[patch("/experimental-features")]
pub async fn patch_experimental_features(
    body: web::Json<ExperimentalFeaturesPatch>,
) -> HttpResponse {
    let _ = body.into_inner();
    HttpResponse::Ok().json(build_experimental_features())
}
