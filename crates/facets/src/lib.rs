//! Faceted search and aggregations for AccelerateSearch.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use errors::AppResult;

/// Distribution of a single facet.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct FacetDistribution {
    /// Field name.
    pub field: String,
    /// `value -> count` for the top `max_values_per_facet` values.
    pub counts: BTreeMap<String, u64>,
    /// Optional numeric statistics (min, max, avg, sum).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<FacetStats>,
}

/// Numeric facet statistics.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct FacetStats {
    /// Minimum value.
    pub min: f64,
    /// Maximum value.
    pub max: f64,
    /// Average value.
    pub avg: f64,
    /// Sum of all values.
    pub sum: f64,
    /// Count of values seen.
    pub count: u64,
}

/// Computes facet distributions from a set of documents.
pub struct FacetEngine {
    max_values_per_facet: usize,
}

impl FacetEngine {
    /// Creates a new engine.
    #[must_use]
    pub fn new(max_values_per_facet: usize) -> Self {
        Self {
            max_values_per_facet: max_values_per_facet.max(1),
        }
    }

    /// Computes facet distributions for the given fields across `docs`.
    pub fn compute(&self, fields: &[String], docs: &[Value]) -> AppResult<Vec<FacetDistribution>> {
        let mut out = Vec::with_capacity(fields.len());
        for field in fields {
            out.push(self.compute_one(field, docs)?);
        }
        Ok(out)
    }

    fn compute_one(&self, field: &str, docs: &[Value]) -> AppResult<FacetDistribution> {
        let mut counts: BTreeMap<String, u64> = BTreeMap::new();
        let mut numeric: Vec<f64> = Vec::new();
        for d in docs {
            if let Some(v) = d.get(field) {
                for value in flatten(v) {
                    match &value {
                        Value::Number(n) => {
                            if let Some(f) = n.as_f64() {
                                numeric.push(f);
                            }
                            *counts.entry(value.to_string()).or_insert(0) += 1;
                        }
                        Value::Bool(b) => {
                            *counts.entry(b.to_string()).or_insert(0) += 1;
                        }
                        Value::String(s) => {
                            *counts.entry(s.clone()).or_insert(0) += 1;
                        }
                        Value::Null => {
                            *counts.entry("null".to_string()).or_insert(0) += 1;
                        }
                        _ => {}
                    }
                }
            }
        }
        // Cap the number of facet values.
        let mut counts_vec: Vec<(String, u64)> = counts.into_iter().collect();
        counts_vec.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        counts_vec.truncate(self.max_values_per_facet);
        let counts: BTreeMap<String, u64> = counts_vec.into_iter().collect();
        let stats = if numeric.is_empty() {
            None
        } else {
            let min = numeric.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = numeric.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let sum: f64 = numeric.iter().sum();
            let count = numeric.len() as u64;
            let avg = sum / count as f64;
            Some(FacetStats {
                min,
                max,
                avg,
                sum,
                count,
            })
        };
        Ok(FacetDistribution {
            field: field.to_string(),
            counts,
            stats,
        })
    }
}

fn flatten(v: &Value) -> Vec<Value> {
    match v {
        Value::Array(arr) => arr.iter().flat_map(flatten).collect(),
        other => vec![other.clone()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn docs() -> Vec<Value> {
        vec![
            serde_json::json!({"category": "books", "price": 10.0}),
            serde_json::json!({"category": "books", "price": 20.0}),
            serde_json::json!({"category": "movies", "price": 15.0}),
        ]
    }

    #[test]
    fn categorical_facet_counts() {
        let engine = FacetEngine::new(10);
        let f = engine.compute(&["category".into()], &docs()).unwrap();
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].counts.get("books"), Some(&2));
        assert_eq!(f[0].counts.get("movies"), Some(&1));
    }

    #[test]
    fn numeric_facet_stats() {
        let engine = FacetEngine::new(10);
        let f = engine.compute(&["price".into()], &docs()).unwrap();
        let s = f[0].stats.as_ref().unwrap();
        assert_eq!(s.min, 10.0);
        assert_eq!(s.max, 20.0);
        assert!((s.avg - 15.0).abs() < 1e-9);
    }

    #[test]
    fn max_values_truncates() {
        let engine = FacetEngine::new(1);
        let f = engine.compute(&["category".into()], &docs()).unwrap();
        assert_eq!(f[0].counts.len(), 1);
    }
}
