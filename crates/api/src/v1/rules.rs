//! Search rules / curated queries endpoints.
//!
//! A search rule attaches a list of pre-defined actions to a query
//! pattern, allowing operators to:
//!
//! * Pin a specific document to the top of the results for a query.
//! * Hide specific documents from the results for a query.
//! * Inject extra filter / sort criteria.

use std::sync::Arc;

use actix_web::{HttpResponse, ResponseError, delete, get, post, web};
use chrono::Utc;
use dashmap::DashMap;
use serde::Serialize;
use tracing::info;
use uuid::Uuid;

use errors::{AppError, AppResult};
use models::{CollectionId, Ruleset};
use storage::{StorageBackend, put_json};

use crate::state::AppState;

/// Storage table used for search rules.
pub const TABLE_RULES: &str = "search_rules";

/// A search ruleset scoped to a single collection.
pub struct RuleService {
    storage: Arc<dyn StorageBackend>,
    cache: DashMap<CollectionId, Ruleset>,
}

impl RuleService {
    /// Creates a new rule service.
    #[must_use]
    pub fn new(storage: Arc<dyn StorageBackend>) -> Self {
        Self {
            storage,
            cache: DashMap::new(),
        }
    }

    /// Loads all rulesets from storage.
    pub async fn load_all(&self) -> AppResult<()> {
        self.cache.clear();
        for k in self.storage.list(TABLE_RULES, "").await? {
            if let Some(bytes) = self.storage.get(TABLE_RULES, &k).await? {
                let r: Ruleset = serde_json::from_slice(&bytes)?;
                self.cache.insert(r.index_uid.clone(), r);
            }
        }
        Ok(())
    }

    /// Returns the ruleset for a collection.
    #[must_use]
    pub fn get(&self, uid: &CollectionId) -> Option<Ruleset> {
        self.cache.get(uid).map(|r| r.value().clone())
    }

    /// Replaces the ruleset for a collection.
    pub async fn put(&self, mut ruleset: Ruleset) -> AppResult<Ruleset> {
        if ruleset.index_uid.as_str().is_empty() {
            return Err(AppError::bad_request("ruleset index_uid is required"));
        }
        let now = Utc::now();
        ruleset.updated_at = now;
        if ruleset.created_at.timestamp() == 0 {
            ruleset.created_at = now;
        }
        self.persist(&ruleset).await?;
        self.cache
            .insert(ruleset.index_uid.clone(), ruleset.clone());
        info!(collection = %ruleset.index_uid, rules = ruleset.rules.len(), "updated search rules");
        Ok(ruleset)
    }

    /// Deletes the ruleset for a collection.
    pub async fn delete(&self, uid: &CollectionId) -> AppResult<bool> {
        let removed = self.cache.remove(uid).is_some();
        if removed {
            self.storage.delete(TABLE_RULES, uid.as_str()).await?;
        }
        Ok(removed)
    }

    async fn persist(&self, ruleset: &Ruleset) -> AppResult<()> {
        put_json(
            self.storage.as_ref(),
            TABLE_RULES,
            ruleset.index_uid.as_str(),
            ruleset,
        )
        .await
    }
}

/// Summary of how a ruleset was applied to a query.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct RulesetSummary {
    /// IDs of rules that matched.
    pub matched_rule_ids: Vec<Uuid>,
    /// Pinned document ids.
    pub pinned_doc_ids: Vec<String>,
    /// Effective query after rule substitution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_query: Option<String>,
    /// Filter injected by the ruleset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub injected_filter: Option<String>,
    /// Sort injected by the ruleset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub injected_sort: Option<Vec<String>>,
}

/// Applies a ruleset to a candidate list of hits for a given query.
/// Returns hits with the ruleset's actions applied (pinned, hidden, filtered, sorted).
#[must_use]
pub fn apply_rules(
    ruleset: Option<&Ruleset>,
    query: &str,
    mut hits: Vec<search::SearchHit>,
) -> (Vec<search::SearchHit>, Option<RulesetSummary>) {
    let Some(rs) = ruleset else {
        return (hits, None);
    };
    let q = query.to_ascii_lowercase();
    let mut pinned_entries: Vec<(usize, String)> = Vec::new();
    let mut hide_ids: Vec<String> = Vec::new();
    let mut injected_filter: Option<String> = None;
    let mut injected_sort: Option<Vec<String>> = None;
    let mut effective_query: Option<String> = None;
    let mut matched_rule_ids: Vec<Uuid> = Vec::new();
    for rule in &rs.rules {
        if !rule.enabled {
            continue;
        }
        if !q.contains(&rule.query.to_ascii_lowercase()) {
            continue;
        }
        matched_rule_ids.push(rule.id);
        for action in &rule.actions {
            match action {
                models::RuleAction::PinnedHit { doc_id, position } => {
                    pinned_entries.push((*position, doc_id.clone()));
                }
                models::RuleAction::HideHits { doc_ids } => hide_ids.extend(doc_ids.clone()),
                models::RuleAction::Query { query } => {
                    effective_query = Some(query.clone());
                }
                models::RuleAction::Filter { filter } => {
                    injected_filter = Some(filter.clone());
                }
                models::RuleAction::Sort { sort } => {
                    injected_sort = Some(sort.clone());
                }
            }
        }
    }
    if !hide_ids.is_empty() {
        hits.retain(|h| {
            !hide_ids.iter().any(|hid| {
                h.document
                    .get("_id")
                    .and_then(serde_json::Value::as_str)
                    .map(|s| s == hid)
                    .unwrap_or(false)
            })
        });
    }
    for (pos, doc_id) in &pinned_entries {
        let placeholder = search::SearchHit {
            document: serde_json::json!({ "_id": doc_id, "_pinned": true }),
            formatted: None,
            ranking_score: None,
        };
        let insert_at = (pos.saturating_sub(1)).min(hits.len());
        hits.insert(insert_at, placeholder);
    }
    if let Some(sort) = injected_sort.clone() {
        sort_hits(&sort, &mut hits);
    }
    let summary = RulesetSummary {
        matched_rule_ids,
        pinned_doc_ids: pinned_entries.into_iter().map(|(_, id)| id).collect(),
        effective_query,
        injected_filter,
        injected_sort,
    };
    (hits, Some(summary))
}

fn sort_hits(sort_fields: &[String], hits: &mut [search::SearchHit]) {
    use serde_json::Value;
    for spec in sort_fields.iter().rev() {
        let desc = spec.ends_with(":desc");
        let field = spec.trim_end_matches(":desc").trim_end_matches(":asc");
        hits.sort_by(|a, b| {
            let va = a.document.get(field).unwrap_or(&Value::Null);
            let vb = b.document.get(field).unwrap_or(&Value::Null);
            let ord = match (va, vb) {
                (Value::Number(x), Value::Number(y)) => x
                    .as_f64()
                    .zip(y.as_f64())
                    .and_then(|(x, y)| x.partial_cmp(&y))
                    .unwrap_or(std::cmp::Ordering::Equal),
                (Value::String(x), Value::String(y)) => x.cmp(y),
                _ => std::cmp::Ordering::Equal,
            };
            if desc { ord.reverse() } else { ord }
        });
    }
}

/// Returns the ruleset for a collection.
#[utoipa::path(
    get,
    path = "/api/v1/indexes/{uid}/settings/rules",
    params(("uid" = String, Path,)),
    responses(
        (status = 200, description = "Search rules", body = Ruleset),
        (status = 404, description = "Collection not found")
    ),
    tag = "rules"
)]
#[get("/indexes/{uid}/settings/rules")]
pub async fn get_rules(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let uid = path.into_inner();
    let id = CollectionId::new(&uid);
    match state.rules.get(&id) {
        Some(r) => HttpResponse::Ok().json(r),
        None => HttpResponse::Ok().json(Ruleset {
            index_uid: id,
            rules: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }),
    }
}

/// Replaces the ruleset for a collection.
#[utoipa::path(
    post,
    path = "/api/v1/indexes/{uid}/settings/rules",
    params(("uid" = String, Path,)),
    request_body = Ruleset,
    responses(
        (status = 200, description = "Rules updated", body = Ruleset)
    ),
    tag = "rules"
)]
#[post("/indexes/{uid}/settings/rules")]
pub async fn put_rules(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<Ruleset>,
) -> HttpResponse {
    let uid = path.into_inner();
    let ruleset = body.into_inner();
    if ruleset.index_uid.as_str() != uid {
        return AppError::bad_request("index_uid in body must match URL").error_response();
    }
    match state.rules.put(ruleset).await {
        Ok(r) => HttpResponse::Ok().json(r),
        Err(e) => e.error_response(),
    }
}

/// Deletes the ruleset for a collection.
#[utoipa::path(
    delete,
    path = "/api/v1/indexes/{uid}/settings/rules",
    params(("uid" = String, Path,)),
    responses(
        (status = 204, description = "Rules deleted")
    ),
    tag = "rules"
)]
#[delete("/indexes/{uid}/settings/rules")]
pub async fn delete_rules(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let uid = path.into_inner();
    let id = CollectionId::new(&uid);
    match state.rules.delete(&id).await {
        Ok(true) => HttpResponse::NoContent().finish(),
        Ok(false) => HttpResponse::NoContent().finish(),
        Err(e) => e.error_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use search::SearchHit;
    use storage::RedbStorage;

    #[tokio::test]
    async fn put_get_delete_round_trip() {
        let backend = Arc::new(RedbStorage::open_temp().unwrap());
        let svc = RuleService::new(backend);
        let id = CollectionId::new("products");
        let ruleset = Ruleset {
            index_uid: id.clone(),
            rules: vec![models::Rule {
                id: Uuid::new_v4(),
                name: "promo".into(),
                enabled: true,
                query: "promo".into(),
                actions: vec![models::RuleAction::PinnedHit {
                    doc_id: "doc1".into(),
                    position: 1,
                }],
            }],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        svc.put(ruleset).await.unwrap();
        let got = svc.get(&id).unwrap();
        assert_eq!(got.rules.len(), 1);
        assert!(svc.delete(&id).await.unwrap());
        assert!(svc.get(&id).is_none());
    }

    #[test]
    fn apply_rules_pins_and_hides() {
        let rs = Ruleset {
            index_uid: CollectionId::new("c"),
            rules: vec![models::Rule {
                id: Uuid::new_v4(),
                name: "p".into(),
                enabled: true,
                query: "phone".into(),
                actions: vec![
                    models::RuleAction::PinnedHit {
                        doc_id: "pinned-1".into(),
                        position: 1,
                    },
                    models::RuleAction::HideHits {
                        doc_ids: vec!["hidden-1".into()],
                    },
                ],
            }],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let hits = vec![
            SearchHit {
                document: serde_json::json!({"_id": "a"}),
                formatted: None,
                ranking_score: None,
            },
            SearchHit {
                document: serde_json::json!({"_id": "hidden-1"}),
                formatted: None,
                ranking_score: None,
            },
        ];
        let (out, summary) = apply_rules(Some(&rs), "buy phone", hits);
        assert!(summary.is_some());
        let s = summary.unwrap();
        assert_eq!(s.pinned_doc_ids, vec!["pinned-1".to_string()]);
        assert_eq!(out.len(), 2, "pinned inserted and hidden removed");
        assert_eq!(out[0].document["_id"], "pinned-1");
    }

    #[test]
    fn apply_rules_no_match_is_passthrough() {
        let hits = vec![SearchHit {
            document: serde_json::json!({"_id": "a"}),
            formatted: None,
            ranking_score: None,
        }];
        let (out, summary) = apply_rules(None, "anything", hits.clone());
        assert!(summary.is_none());
        assert_eq!(out.len(), hits.len());
    }
}
