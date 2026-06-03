//! Search request DTOs.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

/// Search request sent to `POST /collections/{uid}/search`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct SearchRequest {
    /// Free-text query.
    #[serde(default)]
    pub q: Option<String>,
    /// Pagination offset.
    #[serde(default)]
    pub offset: usize,
    /// Pagination limit.
    #[serde(default)]
    pub limit: Option<usize>,
    /// Filter expression.
    #[serde(default)]
    pub filter: Option<String>,
    /// Facet distribution request.
    #[serde(default)]
    pub facets: Option<Vec<String>>,
    /// Field weights for boosting.
    #[serde(default)]
    pub attributes_to_retrieve: Option<Vec<String>>,
    /// Displayed attributes (after projection).
    #[serde(default)]
    pub attributes_to_highlight: Option<Vec<String>>,
    /// Sort specification (`field:asc` or `field:desc`).
    #[serde(default)]
    pub sort: Option<Vec<String>>,
    /// Show ranking score.
    #[serde(default)]
    pub show_ranking_score: bool,
    /// Hybrid semantic ratio.
    #[serde(default)]
    pub hybrid: Option<HybridConfig>,
    /// Vector input for similarity search.
    #[serde(default)]
    pub vector: Option<Vec<f32>>,
    /// Distinct attribute.
    #[serde(default)]
    pub distinct: Option<String>,
    /// Extra metadata.
    #[serde(default)]
    pub extra: BTreeMap<String, Value>,
}

/// Hybrid search configuration.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HybridConfig {
    /// `0.0` = pure keyword, `1.0` = pure vector.
    pub semantic_ratio: f64,
    /// Embedder to use.
    pub embedder: String,
}

/// Search response.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SearchResponse {
    /// The query string (echoed).
    pub query: Option<String>,
    /// Returned hits.
    pub hits: Vec<SearchHit>,
    /// Estimated total number of matches.
    pub estimated_total_hits: u64,
    /// Page offset.
    pub offset: usize,
    /// Page limit.
    pub limit: usize,
    /// Processing time in milliseconds.
    pub processing_time_ms: u128,
    /// Facet distribution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facet_distribution: Option<BTreeMap<String, Value>>,
}

/// A single search hit.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SearchHit {
    /// The matched document.
    pub document: Value,
    /// Highlighted fields, if requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formatted: Option<BTreeMap<String, String>>,
    /// BM25 ranking score.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ranking_score: Option<f64>,
}
