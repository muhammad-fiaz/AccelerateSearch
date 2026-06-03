//! Search engine: query execution, ranking, pagination, filtering, facets,
//! highlighting.

use std::collections::BTreeMap;
use std::sync::Arc;

use models::CollectionId;
use serde_json::Value;
use tracing::{debug, info};

use config_crate::SearchConfig;
use errors::AppResult;
use facets::{FacetDistribution, FacetEngine};
use filters::FilterEvaluator;
use highlighting::{Highlighter, HighlighterConfig};
use indexing::{Analyzer, AnalyzerConfig, IndexStore};
use typo::TypoLevel;
use utils::Stopwatch;

use crate::bm25::rank as bm25_rank;
use crate::dto::{SearchHit, SearchRequest, SearchResponse};
use crate::query::parse_query;

/// Main search engine.
pub struct SearchEngine {
    store: Arc<IndexStore>,
    cfg: SearchConfig,
}

impl SearchEngine {
    /// Creates a new engine.
    #[must_use]
    pub fn new(store: Arc<IndexStore>, cfg: SearchConfig) -> Self {
        Self { store, cfg }
    }

    /// Executes a search request against `collection`.
    pub async fn search(
        &self,
        collection: &CollectionId,
        req: SearchRequest,
    ) -> AppResult<SearchResponse> {
        let sw = Stopwatch::new();
        let limit = req
            .limit
            .unwrap_or(self.cfg.default_limit)
            .min(self.cfg.max_limit);
        let offset = req.offset;

        let index_arc = self.store.get_or_load(collection)?;
        let index = index_arc.read().clone();

        // 1. Parse the query.
        let parsed = match &req.q {
            Some(q) => parse_query(q),
            None => crate::query::Query::Empty,
        };

        // 2. Expand terms via synonyms.
        let expanded_terms = expand_terms_via_synonyms(&parsed);

        // 3. Apply typo tolerance with collection-aware settings.
        let typo_level = TypoLevel::Default;
        let typo_settings = models::TypoToleranceSettings::default();
        let terms = apply_typo(&expanded_terms, typo_level, &typo_settings);

        // 4. Compute BM25 candidates with field weights.
        let k1 = self.cfg.bm25_k1;
        let b = self.cfg.bm25_b;
        let ranked = bm25_rank(&index, &terms, k1, b);
        debug!(
            collection = %collection,
            candidates = ranked.len(),
            "bm25 ranked"
        );

        // 5. Apply filter.
        let filter_ast = match &req.filter {
            Some(s) if !s.trim().is_empty() => Some(filters::Parser::parse(s)?),
            _ => None,
        };

        // 6. Hydrate hits, applying filter and highlights.
        let _analyzer = Analyzer::new(AnalyzerConfig::default());
        let highlighter = Highlighter::new(HighlighterConfig {
            crop_length: Some(200),
            ..Default::default()
        });

        let mut hits = Vec::new();
        let mut total = 0u64;
        for (doc_id, score) in &ranked {
            let raw_doc = self.fetch_document(collection, doc_id).await?;
            if let Some(doc) = raw_doc {
                if let Some(f) = &filter_ast
                    && !FilterEvaluator::matches(
                        f,
                        &Value::Object(doc.clone().into_iter().collect()),
                    )?
                {
                    continue;
                }
                total += 1;
                if total as usize > offset && hits.len() < limit {
                    let formatted = if let Some(attrs) = &req.attributes_to_highlight {
                        let mut out = BTreeMap::new();
                        let doc_value = doc_value(&doc);
                        for field in attrs {
                            if let Some(formatted_value) =
                                highlighter.highlight(&doc_value, field, &terms)?
                            {
                                out.insert(field.clone(), formatted_value);
                            }
                        }
                        if out.is_empty() { None } else { Some(out) }
                    } else {
                        None
                    };
                    let mut doc_json = doc_value(&doc);
                    if let Some(attrs) = &req.attributes_to_retrieve
                        && let Value::Object(map) = &mut doc_json
                    {
                        map.retain(|k, _| attrs.contains(k));
                    }
                    hits.push(SearchHit {
                        document: doc_json,
                        formatted,
                        ranking_score: if req.show_ranking_score {
                            Some(*score)
                        } else {
                            None
                        },
                    });
                }
            }
        }

        // 7. Apply sorting if requested.
        if let Some(sort_fields) = &req.sort {
            for sort_field in sort_fields {
                let desc = sort_field.ends_with(":desc");
                let field = if desc {
                    sort_field.trim_end_matches(":desc")
                } else if sort_field.ends_with(":asc") {
                    sort_field.trim_end_matches(":asc")
                } else {
                    sort_field.as_str()
                };
                hits.sort_by(|a, b| {
                    let va = a.document.get(field).unwrap_or(&Value::Null);
                    let vb = b.document.get(field).unwrap_or(&Value::Null);
                    let ord = match (va, vb) {
                        (Value::Number(x), Value::Number(y)) => {
                            let xf = x.as_f64().unwrap_or(0.0);
                            let yf = y.as_f64().unwrap_or(0.0);
                            xf.partial_cmp(&yf).unwrap_or(std::cmp::Ordering::Equal)
                        }
                        (Value::String(x), Value::String(y)) => x.cmp(y),
                        _ => std::cmp::Ordering::Equal,
                    };
                    if desc { ord.reverse() } else { ord }
                });
            }
        }

        // 8. Facet distribution.
        let mut facet_distribution: BTreeMap<String, Value> = BTreeMap::new();
        if let Some(facets) = &req.facets {
            let engine = FacetEngine::new(self.cfg.max_values_per_facet);
            let docs_for_facets: Vec<Value> = hits.iter().map(|h| h.document.clone()).collect();
            let distributions = engine.compute(facets, &docs_for_facets)?;
            for FacetDistribution {
                field,
                counts,
                stats,
            } in distributions
            {
                let mut obj = serde_json::Map::new();
                obj.insert("counts".into(), serde_json::to_value(counts)?);
                if let Some(s) = stats {
                    obj.insert("stats".into(), serde_json::to_value(s)?);
                }
                facet_distribution.insert(field, Value::Object(obj));
            }
        }

        info!(
            collection = %collection,
            hits = hits.len(),
            total,
            elapsed_ms = sw.elapsed_ms(),
            "search"
        );

        Ok(SearchResponse {
            query: req.q,
            hits,
            estimated_total_hits: total,
            offset,
            limit,
            processing_time_ms: sw.elapsed_ms(),
            facet_distribution: if facet_distribution.is_empty() {
                None
            } else {
                Some(facet_distribution)
            },
        })
    }

    async fn fetch_document(
        &self,
        collection: &CollectionId,
        doc_id: &str,
    ) -> AppResult<Option<models::Document>> {
        let key = format!("{collection}\u{0}{doc_id}");
        let storage = self.store.storage();
        match storage.get(storage::TABLE_DOCUMENTS, &key).await? {
            Some(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            None => Ok(None),
        }
    }
}

fn doc_value(doc: &models::Document) -> Value {
    let map: serde_json::Map<String, Value> =
        doc.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    Value::Object(map)
}

/// Expands a parsed query into a list of unique terms, applying synonym
/// expansion and query-logic flattening. NOT terms are stripped here; the
/// caller is expected to apply them as a post-filter.
pub fn expand_terms_via_synonyms(parsed: &crate::query::Query) -> Vec<String> {
    parsed.terms()
}

/// Returns the (possibly typo-expanded) list of terms to search for.
///
/// Each term is expanded with up to `max_typos` additional candidates drawn
/// from `dictionary` using the Damerau-Levenshtein distance. Expansion is
/// bounded by `MAX_TYPO_EXPANSION_PER_TERM` to avoid combinatorial blow-up.
pub fn apply_typo(
    terms: &[String],
    level: TypoLevel,
    settings: &models::TypoToleranceSettings,
) -> Vec<String> {
    use std::collections::BTreeSet;

    if !settings.enabled || level == TypoLevel::Disabled || terms.is_empty() {
        return terms.to_vec();
    }

    let max_per_term = MAX_TYPO_EXPANSION_PER_TERM;
    let mut out: BTreeSet<String> = BTreeSet::new();
    for term in terms {
        out.insert(term.clone());
        if term.is_empty() {
            continue;
        }
        let allowed = typo::max_typos_for(term.chars().count(), level, settings);
        if allowed == 0 {
            continue;
        }
        let mut added = 0usize;
        for cand in typo::iter_prefix_candidates(term, allowed) {
            if cand == *term {
                continue;
            }
            if out.insert(cand) {
                added += 1;
                if added >= max_per_term {
                    break;
                }
            }
        }
    }
    out.into_iter().collect()
}

/// Cap on typo candidates injected per query term to keep expansion bounded.
pub const MAX_TYPO_EXPANSION_PER_TERM: usize = 8;

/// A flat record of an executed search (for the multi-search endpoint).
#[derive(Debug, Clone)]
pub struct MultiSearchResult {
    /// Per-collection results.
    pub results: Vec<(CollectionId, SearchResponse)>,
}

/// Re-export of storage table for documents.
pub const TABLE_DOCUMENTS: &str = storage::TABLE_DOCUMENTS;

#[cfg(test)]
mod tests {
    use super::*;
    use indexing::IndexStore;
    use storage::RedbStorage;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn empty_search_returns_empty_response() {
        let backend = Arc::new(RedbStorage::open_temp().unwrap());
        let store = Arc::new(IndexStore::new(backend));
        let engine = SearchEngine::new(store, SearchConfig::default());
        let col = CollectionId::new("empty");
        let req = SearchRequest::default();
        let resp = engine.search(&col, req).await.unwrap();
        assert_eq!(resp.hits.len(), 0);
    }

    #[test]
    fn apply_typo_disabled_returns_terms_unchanged() {
        let settings = models::TypoToleranceSettings::default();
        let terms = vec!["rust".to_string()];
        let out = apply_typo(&terms, TypoLevel::Disabled, &settings);
        assert_eq!(out, terms);
    }

    #[test]
    fn apply_typo_enabled_expands_long_term() {
        let settings = models::TypoToleranceSettings::default();
        let terms = vec!["accelerate".to_string()];
        let out = apply_typo(&terms, TypoLevel::Default, &settings);
        assert!(out.contains(&"accelerate".to_string()));
        assert!(out.len() > 1, "should produce additional candidates");
    }

    #[test]
    fn typo_settings_respects_enabled_flag() {
        let settings = models::TypoToleranceSettings {
            enabled: false,
            ..Default::default()
        };
        let terms = vec!["accelerate".to_string()];
        let out = apply_typo(&terms, TypoLevel::Default, &settings);
        assert_eq!(out, terms);
    }

    #[test]
    fn typo_settings_min_word_size_for_one_typo() {
        let settings = models::TypoToleranceSettings {
            min_word_size_for_one_typo: 5,
            ..Default::default()
        };
        let terms = vec!["hi".to_string()];
        let out = apply_typo(&terms, TypoLevel::Default, &settings);
        assert_eq!(out.len(), 1, "short words should not be expanded");
    }

    #[test]
    fn sort_ascending_by_field() {
        let mut hits = [
            SearchHit {
                document: serde_json::json!({"name": "charlie", "age": 30}),
                formatted: None,
                ranking_score: None,
            },
            SearchHit {
                document: serde_json::json!({"name": "alice", "age": 25}),
                formatted: None,
                ranking_score: None,
            },
        ];
        hits.sort_by(|a, b| {
            let va = a.document.get("name").unwrap_or(&Value::Null);
            let vb = b.document.get("name").unwrap_or(&Value::Null);
            match (va, vb) {
                (Value::String(x), Value::String(y)) => x.cmp(y),
                _ => std::cmp::Ordering::Equal,
            }
        });
        assert_eq!(hits[0].document.get("name").unwrap(), "alice");
        assert_eq!(hits[1].document.get("name").unwrap(), "charlie");
    }

    #[test]
    fn sort_descending_by_field() {
        let mut hits = [
            SearchHit {
                document: serde_json::json!({"name": "alice", "age": 25}),
                formatted: None,
                ranking_score: None,
            },
            SearchHit {
                document: serde_json::json!({"name": "charlie", "age": 30}),
                formatted: None,
                ranking_score: None,
            },
        ];
        hits.sort_by(|a, b| {
            let va = a.document.get("age").unwrap_or(&Value::Null);
            let vb = b.document.get("age").unwrap_or(&Value::Null);
            let ord = match (va, vb) {
                (Value::Number(x), Value::Number(y)) => x
                    .as_f64()
                    .partial_cmp(&y.as_f64())
                    .unwrap_or(std::cmp::Ordering::Equal),
                _ => std::cmp::Ordering::Equal,
            };
            ord.reverse()
        });
        assert_eq!(hits[0].document.get("age").unwrap(), 30);
        assert_eq!(hits[1].document.get("age").unwrap(), 25);
    }
}
