//! Hybrid search: merges BM25 keyword ranking and vector similarity ranking
//! using Reciprocal Rank Fusion (RRF).
//!
//! RRF score for document `d` is:
//!
//! ```text
//! score(d) = sum( w_i / (k + rank_i(d)) )
//! ```
//!
//! where `k` is a configurable constant (default 60), `w_i` is the weight
//! for the i-th ranking list, and `rank_i` is the 1-based rank of `d` in
//! that list.

use std::collections::HashMap;

use models::DocumentId;
use tracing::debug;

use errors::AppResult;

/// Default RRF constant.
pub const RRF_K: f64 = 60.0;

/// A single ranking entry: a document identifier and a score.
#[derive(Debug, Clone)]
pub struct Ranked {
    /// Document id.
    pub id: DocumentId,
    /// Score (higher = better).
    pub score: f64,
}

/// Configuration for hybrid fusion.
#[derive(Debug, Clone)]
pub struct FusionConfig {
    /// RRF constant `k`. Higher values reduce the impact of rank differences.
    pub rrf_k: f64,
    /// Weight for the keyword ranking list.
    pub keyword_weight: f64,
    /// Weight for the vector ranking list.
    pub vector_weight: f64,
}

impl Default for FusionConfig {
    fn default() -> Self {
        Self {
            rrf_k: RRF_K,
            keyword_weight: 1.0,
            vector_weight: 1.0,
        }
    }
}

/// Normalizes scores in a ranking list to [0, 1] range.
///
/// If the list is empty or all scores are the same, the scores are left unchanged.
pub fn normalize_scores(ranked: &mut [Ranked]) {
    if ranked.is_empty() {
        return;
    }
    let max_score = ranked.iter().map(|r| r.score).fold(f64::NEG_INFINITY, f64::max);
    let min_score = ranked.iter().map(|r| r.score).fold(f64::INFINITY, f64::min);
    let range = max_score - min_score;
    if range > f64::EPSILON {
        for r in ranked.iter_mut() {
            r.score = (r.score - min_score) / range;
        }
    }
}

/// Fuses multiple ranking lists using weighted RRF.
///
/// `config` controls the RRF constant and relative weights of each list.
/// Returns documents sorted by fused score descending.
#[must_use]
pub fn fuse_with_config(
    keyword: &[Ranked],
    vector: &[Ranked],
    config: &FusionConfig,
) -> Vec<(DocumentId, f64)> {
    let total_weight = config.keyword_weight + config.vector_weight;
    if total_weight <= f64::EPSILON {
        return Vec::new();
    }
    let kw_weight = config.keyword_weight / total_weight;
    let vec_weight = config.vector_weight / total_weight;
    let k = config.rrf_k;

    let mut scores: HashMap<&str, f64> = HashMap::with_capacity(keyword.len() + vector.len());

    for (i, r) in keyword.iter().enumerate() {
        let rank = (i + 1) as f64;
        let contribution = kw_weight / (k + rank);
        *scores.entry(r.id.as_str()).or_insert(0.0) += contribution;
    }

    for (i, r) in vector.iter().enumerate() {
        let rank = (i + 1) as f64;
        let contribution = vec_weight / (k + rank);
        *scores.entry(r.id.as_str()).or_insert(0.0) += contribution;
    }

    let mut out: Vec<(DocumentId, f64)> = scores
        .into_iter()
        .map(|(id, score)| (DocumentId::new(id), score))
        .collect();

    out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    debug!(count = out.len(), "fused ranking with RRF");
    out
}

/// Fuses multiple ranking lists using RRF with default configuration.
///
/// `semantic_ratio` controls the relative weight of the vector result list
/// vs. the keyword result list. `0.0` ignores the vector list entirely,
/// `1.0` ignores the keyword list entirely. Intermediate values blend
/// the two lists proportionally.
#[must_use]
pub fn fuse(keyword: &[Ranked], vector: &[Ranked], semantic_ratio: f64) -> Vec<(DocumentId, f64)> {
    let ratio = semantic_ratio.clamp(0.0, 1.0);
    let config = FusionConfig {
        keyword_weight: 1.0 - ratio,
        vector_weight: ratio,
        ..Default::default()
    };
    fuse_with_config(keyword, vector, &config)
}

/// Combines keyword scores and vector scores into a single ranking.
///
/// This is the async entry point for hybrid search. It normalizes scores
/// from both lists, fuses them using RRF, and returns the top-k results.
pub async fn hybrid_search(
    keyword: Vec<Ranked>,
    vector: Vec<Ranked>,
    semantic_ratio: f64,
    limit: usize,
) -> AppResult<Vec<(DocumentId, f64)>> {
    let fused = fuse(&keyword, &vector, semantic_ratio);
    Ok(fused.into_iter().take(limit).collect())
}

/// Batch hybrid search: processes multiple queries in parallel.
///
/// Each query is a `(keyword_results, vector_results)` pair. Returns
/// a vector of fused results for each query.
pub async fn batch_hybrid_search(
    queries: Vec<(Vec<Ranked>, Vec<Ranked>)>,
    semantic_ratio: f64,
    limit: usize,
) -> AppResult<Vec<Vec<(DocumentId, f64)>>> {
    let mut results = Vec::with_capacity(queries.len());
    for (keyword, vector) in queries {
        let fused = hybrid_search(keyword, vector, semantic_ratio, limit).await?;
        results.push(fused);
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(id: &str, score: f64) -> Ranked {
        Ranked {
            id: DocumentId::new(id),
            score,
        }
    }

    #[test]
    fn keyword_only_with_zero_ratio() {
        let kw = vec![r("a", 1.0), r("b", 0.5)];
        let v = vec![r("c", 1.0)];
        let out = fuse(&kw, &v, 0.0);
        assert_eq!(out[0].0.as_str(), "a");
    }

    #[test]
    fn vector_only_with_full_ratio() {
        let kw = vec![r("a", 1.0)];
        let v = vec![r("c", 1.0), r("d", 0.5)];
        let out = fuse(&kw, &v, 1.0);
        assert_eq!(out[0].0.as_str(), "c");
    }

    #[test]
    fn mixed_results_are_fused() {
        let kw = vec![r("a", 1.0), r("b", 0.5)];
        let v = vec![r("b", 0.9), r("c", 0.8)];
        let out = fuse(&kw, &v, 0.5);
        assert_eq!(out[0].0.as_str(), "b");
    }

    #[tokio::test]
    async fn hybrid_search_truncates() {
        let kw = vec![r("a", 1.0), r("b", 0.5), r("c", 0.2)];
        let v = vec![r("d", 0.9)];
        let out = hybrid_search(kw, v, 0.5, 2).await.unwrap();
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn fuse_with_config_uses_weights() {
        let kw = vec![r("a", 1.0)];
        let v = vec![r("b", 1.0)];
        let config = FusionConfig {
            keyword_weight: 2.0,
            vector_weight: 1.0,
            ..Default::default()
        };
        let out = fuse_with_config(&kw, &v, &config);
        assert_eq!(out[0].0.as_str(), "a");
    }

    #[test]
    fn fuse_with_config_custom_k() {
        let kw = vec![r("a", 1.0), r("b", 0.5)];
        let v = vec![r("c", 1.0)];
        let config = FusionConfig {
            rrf_k: 1.0,
            ..Default::default()
        };
        let out = fuse_with_config(&kw, &v, &config);
        assert!(!out.is_empty());
    }

    #[test]
    fn fuse_empty_lists() {
        let kw: Vec<Ranked> = vec![];
        let v: Vec<Ranked> = vec![];
        let out = fuse(&kw, &v, 0.5);
        assert!(out.is_empty());
    }

    #[test]
    fn fuse_zero_weight_lists() {
        let kw = vec![r("a", 1.0)];
        let v = vec![r("b", 1.0)];
        let config = FusionConfig {
            keyword_weight: 0.0,
            vector_weight: 0.0,
            ..Default::default()
        };
        let out = fuse_with_config(&kw, &v, &config);
        assert!(out.is_empty());
    }

    #[test]
    fn normalize_scores_scales_to_unit_range() {
        let mut ranked = vec![r("a", 10.0), r("b", 20.0), r("c", 30.0)];
        normalize_scores(&mut ranked);
        assert!((ranked[0].score - 0.0).abs() < 1e-6);
        assert!((ranked[2].score - 1.0).abs() < 1e-6);
    }

    #[test]
    fn normalize_scores_empty_is_noop() {
        let mut ranked: Vec<Ranked> = vec![];
        normalize_scores(&mut ranked);
        assert!(ranked.is_empty());
    }

    #[test]
    fn normalize_scores_same_values() {
        let mut ranked = vec![r("a", 5.0), r("b", 5.0)];
        normalize_scores(&mut ranked);
        assert!((ranked[0].score - 5.0).abs() < 1e-6);
    }

    #[tokio::test]
    async fn batch_hybrid_search_works() {
        let queries = vec![
            (vec![r("a", 1.0)], vec![r("b", 1.0)]),
            (vec![r("c", 1.0)], vec![r("d", 1.0)]),
        ];
        let results = batch_hybrid_search(queries, 0.5, 1).await.unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].len(), 1);
        assert_eq!(results[1].len(), 1);
    }
}
