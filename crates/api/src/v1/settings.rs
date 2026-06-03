//! Per-setting REST endpoints for collections.
//!
//! Each "leaf" of `CollectionSettings` has its own GET/PUT/DELETE trio so
//! clients can manage individual settings without round-tripping the full
//! `CollectionSettings` blob. The endpoints all funnel into
//! `CollectionStore::update_settings` so persistence and concurrency
//! behaviour is identical to the aggregate `PATCH /settings` endpoint.

use actix_web::{HttpResponse, ResponseError, delete, get, put, web};
use serde::{Deserialize, Serialize};

use errors::AppError;
use models::{CollectionId, CollectionSettings, TypoToleranceSettings};

use crate::state::AppState;

/// Settings wrapper: `{"filterableAttributes": [...]}`.
#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct FilterableAttributes {
    /// The list of filterable attribute names.
    pub filterable_attributes: Vec<String>,
}

/// Settings wrapper: `{"sortableAttributes": [...]}`.
#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct SortableAttributes {
    /// The list of sortable attribute names.
    pub sortable_attributes: Vec<String>,
}

/// Settings wrapper: `{"searchableAttributes": [...]}`.
#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct SearchableAttributes {
    /// The list of searchable attribute names.
    pub searchable_attributes: Vec<String>,
}

/// Settings wrapper: `{"displayedAttributes": [...]}`.
#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct DisplayedAttributes {
    /// The list of displayed attribute names.
    pub displayed_attributes: Vec<String>,
}

/// Settings wrapper: `{"stopWords": [...]}`.
#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct StopWordsPayload {
    /// The list of stop words.
    pub stop_words: Vec<String>,
}

/// Settings wrapper: `{"rankingRules": [...]}`.
#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct RankingRules {
    /// The list of ranking rule names.
    pub ranking_rules: Vec<String>,
}

/// Settings wrapper: `{"distinctField": "..."}`.
#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct DistinctField {
    /// The field name to deduplicate on. `None` disables de-duplication.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distinct_field: Option<String>,
}

fn not_found(uid: &str) -> HttpResponse {
    AppError::not_found(format!("collection '{uid}' not found")).error_response()
}

fn load_or_404(state: &AppState, uid: &str) -> Result<CollectionSettings, HttpResponse> {
    let id = CollectionId::new(uid);
    match state.collections.get(&id) {
        Some(c) => Ok(c.settings),
        None => Err(not_found(uid)),
    }
}

async fn save(state: &AppState, uid: &str, new_settings: CollectionSettings) -> HttpResponse {
    let id = CollectionId::new(uid);
    match state.collections.update_settings(&id, new_settings).await {
        Ok(c) => HttpResponse::Ok().json(c.settings),
        Err(e) => e.error_response(),
    }
}

// ---------------- filterable-attributes ----------------

/// Returns the filterable attributes.
#[utoipa::path(
    get,
    path = "/api/v1/collections/{uid}/settings/filterable-attributes",
    params(("uid" = String, Path,)),
    responses(
        (status = 200, description = "Filterable attributes", body = FilterableAttributes),
        (status = 404, description = "Collection not found")
    ),
    tag = "collections"
)]
#[get("/collections/{uid}/settings/filterable-attributes")]
pub async fn get_filterable_attributes(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let uid = path.into_inner();
    match load_or_404(&state, &uid) {
        Ok(s) => HttpResponse::Ok().json(FilterableAttributes {
            filterable_attributes: s.filterable_attributes,
        }),
        Err(r) => r,
    }
}

/// Sets the filterable attributes.
#[utoipa::path(
    put,
    path = "/api/v1/collections/{uid}/settings/filterable-attributes",
    params(("uid" = String, Path,)),
    request_body = FilterableAttributes,
    responses(
        (status = 200, description = "Filterable attributes updated", body = FilterableAttributes),
        (status = 404, description = "Collection not found")
    ),
    tag = "collections"
)]
#[put("/collections/{uid}/settings/filterable-attributes")]
pub async fn put_filterable_attributes(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<FilterableAttributes>,
) -> HttpResponse {
    let uid = path.into_inner();
    let mut s = match load_or_404(&state, &uid) {
        Ok(s) => s,
        Err(r) => return r,
    };
    s.filterable_attributes = body.into_inner().filterable_attributes;
    save(&state, &uid, s).await
}

/// Resets the filterable attributes to an empty list.
#[utoipa::path(
    delete,
    path = "/api/v1/collections/{uid}/settings/filterable-attributes",
    params(("uid" = String, Path,)),
    responses(
        (status = 200, description = "Filterable attributes reset", body = FilterableAttributes),
        (status = 404, description = "Collection not found")
    ),
    tag = "collections"
)]
#[delete("/collections/{uid}/settings/filterable-attributes")]
pub async fn delete_filterable_attributes(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let uid = path.into_inner();
    let mut s = match load_or_404(&state, &uid) {
        Ok(s) => s,
        Err(r) => return r,
    };
    s.filterable_attributes.clear();
    save(&state, &uid, s).await
}

// ---------------- sortable-attributes ----------------

/// Returns the sortable attributes.
#[utoipa::path(
    get,
    path = "/api/v1/collections/{uid}/settings/sortable-attributes",
    params(("uid" = String, Path,)),
    responses(
        (status = 200, description = "Sortable attributes", body = SortableAttributes),
        (status = 404, description = "Collection not found")
    ),
    tag = "collections"
)]
#[get("/collections/{uid}/settings/sortable-attributes")]
pub async fn get_sortable_attributes(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let uid = path.into_inner();
    match load_or_404(&state, &uid) {
        Ok(s) => HttpResponse::Ok().json(SortableAttributes {
            sortable_attributes: s.sortable_attributes,
        }),
        Err(r) => r,
    }
}

/// Sets the sortable attributes.
#[utoipa::path(
    put,
    path = "/api/v1/collections/{uid}/settings/sortable-attributes",
    params(("uid" = String, Path,)),
    request_body = SortableAttributes,
    responses(
        (status = 200, description = "Sortable attributes updated", body = SortableAttributes),
        (status = 404, description = "Collection not found")
    ),
    tag = "collections"
)]
#[put("/collections/{uid}/settings/sortable-attributes")]
pub async fn put_sortable_attributes(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<SortableAttributes>,
) -> HttpResponse {
    let uid = path.into_inner();
    let mut s = match load_or_404(&state, &uid) {
        Ok(s) => s,
        Err(r) => return r,
    };
    s.sortable_attributes = body.into_inner().sortable_attributes;
    save(&state, &uid, s).await
}

/// Resets the sortable attributes.
#[utoipa::path(
    delete,
    path = "/api/v1/collections/{uid}/settings/sortable-attributes",
    params(("uid" = String, Path,)),
    responses(
        (status = 200, description = "Sortable attributes reset", body = SortableAttributes),
        (status = 404, description = "Collection not found")
    ),
    tag = "collections"
)]
#[delete("/collections/{uid}/settings/sortable-attributes")]
pub async fn delete_sortable_attributes(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let uid = path.into_inner();
    let mut s = match load_or_404(&state, &uid) {
        Ok(s) => s,
        Err(r) => return r,
    };
    s.sortable_attributes.clear();
    save(&state, &uid, s).await
}

// ---------------- searchable-attributes ----------------

/// Returns the searchable attributes.
#[utoipa::path(
    get,
    path = "/api/v1/collections/{uid}/settings/searchable-attributes",
    params(("uid" = String, Path,)),
    responses(
        (status = 200, description = "Searchable attributes", body = SearchableAttributes),
        (status = 404, description = "Collection not found")
    ),
    tag = "collections"
)]
#[get("/collections/{uid}/settings/searchable-attributes")]
pub async fn get_searchable_attributes(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let uid = path.into_inner();
    match load_or_404(&state, &uid) {
        Ok(s) => HttpResponse::Ok().json(SearchableAttributes {
            searchable_attributes: s.searchable_attributes,
        }),
        Err(r) => r,
    }
}

/// Sets the searchable attributes.
#[utoipa::path(
    put,
    path = "/api/v1/collections/{uid}/settings/searchable-attributes",
    params(("uid" = String, Path,)),
    request_body = SearchableAttributes,
    responses(
        (status = 200, description = "Searchable attributes updated", body = SearchableAttributes),
        (status = 404, description = "Collection not found")
    ),
    tag = "collections"
)]
#[put("/collections/{uid}/settings/searchable-attributes")]
pub async fn put_searchable_attributes(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<SearchableAttributes>,
) -> HttpResponse {
    let uid = path.into_inner();
    let mut s = match load_or_404(&state, &uid) {
        Ok(s) => s,
        Err(r) => return r,
    };
    s.searchable_attributes = body.into_inner().searchable_attributes;
    save(&state, &uid, s).await
}

/// Resets the searchable attributes.
#[utoipa::path(
    delete,
    path = "/api/v1/collections/{uid}/settings/searchable-attributes",
    params(("uid" = String, Path,)),
    responses(
        (status = 200, description = "Searchable attributes reset", body = SearchableAttributes),
        (status = 404, description = "Collection not found")
    ),
    tag = "collections"
)]
#[delete("/collections/{uid}/settings/searchable-attributes")]
pub async fn delete_searchable_attributes(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let uid = path.into_inner();
    let mut s = match load_or_404(&state, &uid) {
        Ok(s) => s,
        Err(r) => return r,
    };
    s.searchable_attributes.clear();
    save(&state, &uid, s).await
}

// ---------------- displayed-attributes ----------------

/// Returns the displayed attributes.
#[utoipa::path(
    get,
    path = "/api/v1/collections/{uid}/settings/displayed-attributes",
    params(("uid" = String, Path,)),
    responses(
        (status = 200, description = "Displayed attributes", body = DisplayedAttributes),
        (status = 404, description = "Collection not found")
    ),
    tag = "collections"
)]
#[get("/collections/{uid}/settings/displayed-attributes")]
pub async fn get_displayed_attributes(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let uid = path.into_inner();
    match load_or_404(&state, &uid) {
        Ok(s) => HttpResponse::Ok().json(DisplayedAttributes {
            displayed_attributes: s.displayed_attributes,
        }),
        Err(r) => r,
    }
}

/// Sets the displayed attributes.
#[utoipa::path(
    put,
    path = "/api/v1/collections/{uid}/settings/displayed-attributes",
    params(("uid" = String, Path,)),
    request_body = DisplayedAttributes,
    responses(
        (status = 200, description = "Displayed attributes updated", body = DisplayedAttributes),
        (status = 404, description = "Collection not found")
    ),
    tag = "collections"
)]
#[put("/collections/{uid}/settings/displayed-attributes")]
pub async fn put_displayed_attributes(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<DisplayedAttributes>,
) -> HttpResponse {
    let uid = path.into_inner();
    let mut s = match load_or_404(&state, &uid) {
        Ok(s) => s,
        Err(r) => return r,
    };
    s.displayed_attributes = body.into_inner().displayed_attributes;
    save(&state, &uid, s).await
}

/// Resets the displayed attributes.
#[utoipa::path(
    delete,
    path = "/api/v1/collections/{uid}/settings/displayed-attributes",
    params(("uid" = String, Path,)),
    responses(
        (status = 200, description = "Displayed attributes reset", body = DisplayedAttributes),
        (status = 404, description = "Collection not found")
    ),
    tag = "collections"
)]
#[delete("/collections/{uid}/settings/displayed-attributes")]
pub async fn delete_displayed_attributes(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let uid = path.into_inner();
    let mut s = match load_or_404(&state, &uid) {
        Ok(s) => s,
        Err(r) => return r,
    };
    s.displayed_attributes.clear();
    save(&state, &uid, s).await
}

// ---------------- stop-words ----------------

/// Returns the stop words.
#[utoipa::path(
    get,
    path = "/api/v1/collections/{uid}/settings/stop-words",
    params(("uid" = String, Path,)),
    responses(
        (status = 200, description = "Stop words", body = StopWordsPayload),
        (status = 404, description = "Collection not found")
    ),
    tag = "collections"
)]
#[get("/collections/{uid}/settings/stop-words")]
pub async fn get_stop_words(state: web::Data<AppState>, path: web::Path<String>) -> HttpResponse {
    let uid = path.into_inner();
    match load_or_404(&state, &uid) {
        Ok(s) => HttpResponse::Ok().json(StopWordsPayload {
            stop_words: s.stop_words,
        }),
        Err(r) => r,
    }
}

/// Sets the stop words.
#[utoipa::path(
    put,
    path = "/api/v1/collections/{uid}/settings/stop-words",
    params(("uid" = String, Path,)),
    request_body = StopWordsPayload,
    responses(
        (status = 200, description = "Stop words updated", body = StopWordsPayload),
        (status = 404, description = "Collection not found")
    ),
    tag = "collections"
)]
#[put("/collections/{uid}/settings/stop-words")]
pub async fn put_stop_words(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<StopWordsPayload>,
) -> HttpResponse {
    let uid = path.into_inner();
    let mut s = match load_or_404(&state, &uid) {
        Ok(s) => s,
        Err(r) => return r,
    };
    s.stop_words = body.into_inner().stop_words;
    save(&state, &uid, s).await
}

/// Resets the stop words.
#[utoipa::path(
    delete,
    path = "/api/v1/collections/{uid}/settings/stop-words",
    params(("uid" = String, Path,)),
    responses(
        (status = 200, description = "Stop words reset", body = StopWordsPayload),
        (status = 404, description = "Collection not found")
    ),
    tag = "collections"
)]
#[delete("/collections/{uid}/settings/stop-words")]
pub async fn delete_stop_words(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let uid = path.into_inner();
    let mut s = match load_or_404(&state, &uid) {
        Ok(s) => s,
        Err(r) => return r,
    };
    s.stop_words.clear();
    save(&state, &uid, s).await
}

// ---------------- ranking-rules ----------------

/// Returns the ranking rules.
#[utoipa::path(
    get,
    path = "/api/v1/collections/{uid}/settings/ranking-rules",
    params(("uid" = String, Path,)),
    responses(
        (status = 200, description = "Ranking rules", body = RankingRules),
        (status = 404, description = "Collection not found")
    ),
    tag = "collections"
)]
#[get("/collections/{uid}/settings/ranking-rules")]
pub async fn get_ranking_rules(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let uid = path.into_inner();
    match load_or_404(&state, &uid) {
        Ok(s) => HttpResponse::Ok().json(RankingRules {
            ranking_rules: s.ranking_rules,
        }),
        Err(r) => r,
    }
}

/// Sets the ranking rules.
#[utoipa::path(
    put,
    path = "/api/v1/collections/{uid}/settings/ranking-rules",
    params(("uid" = String, Path,)),
    request_body = RankingRules,
    responses(
        (status = 200, description = "Ranking rules updated", body = RankingRules),
        (status = 404, description = "Collection not found")
    ),
    tag = "collections"
)]
#[put("/collections/{uid}/settings/ranking-rules")]
pub async fn put_ranking_rules(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<RankingRules>,
) -> HttpResponse {
    let uid = path.into_inner();
    let mut s = match load_or_404(&state, &uid) {
        Ok(s) => s,
        Err(r) => return r,
    };
    s.ranking_rules = body.into_inner().ranking_rules;
    save(&state, &uid, s).await
}

/// Resets the ranking rules to the default order.
#[utoipa::path(
    delete,
    path = "/api/v1/collections/{uid}/settings/ranking-rules",
    params(("uid" = String, Path,)),
    responses(
        (status = 200, description = "Ranking rules reset", body = RankingRules),
        (status = 404, description = "Collection not found")
    ),
    tag = "collections"
)]
#[delete("/collections/{uid}/settings/ranking-rules")]
pub async fn delete_ranking_rules(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let uid = path.into_inner();
    let mut s = match load_or_404(&state, &uid) {
        Ok(s) => s,
        Err(r) => return r,
    };
    s.ranking_rules.clear();
    save(&state, &uid, s).await
}

// ---------------- typo-tolerance ----------------

/// Returns the typo tolerance settings.
#[utoipa::path(
    get,
    path = "/api/v1/collections/{uid}/settings/typo-tolerance",
    params(("uid" = String, Path,)),
    responses(
        (status = 200, description = "Typo tolerance", body = TypoToleranceSettings),
        (status = 404, description = "Collection not found")
    ),
    tag = "collections"
)]
#[get("/collections/{uid}/settings/typo-tolerance")]
pub async fn get_typo_tolerance(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let uid = path.into_inner();
    match load_or_404(&state, &uid) {
        Ok(s) => HttpResponse::Ok().json(s.typo_tolerance),
        Err(r) => r,
    }
}

/// Sets the typo tolerance settings.
#[utoipa::path(
    put,
    path = "/api/v1/collections/{uid}/settings/typo-tolerance",
    params(("uid" = String, Path,)),
    request_body = TypoToleranceSettings,
    responses(
        (status = 200, description = "Typo tolerance updated", body = TypoToleranceSettings),
        (status = 404, description = "Collection not found")
    ),
    tag = "collections"
)]
#[put("/collections/{uid}/settings/typo-tolerance")]
pub async fn put_typo_tolerance(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<TypoToleranceSettings>,
) -> HttpResponse {
    let uid = path.into_inner();
    let mut s = match load_or_404(&state, &uid) {
        Ok(s) => s,
        Err(r) => return r,
    };
    s.typo_tolerance = body.into_inner();
    save(&state, &uid, s).await
}

/// Resets the typo tolerance to defaults.
#[utoipa::path(
    delete,
    path = "/api/v1/collections/{uid}/settings/typo-tolerance",
    params(("uid" = String, Path,)),
    responses(
        (status = 200, description = "Typo tolerance reset", body = TypoToleranceSettings),
        (status = 404, description = "Collection not found")
    ),
    tag = "collections"
)]
#[delete("/collections/{uid}/settings/typo-tolerance")]
pub async fn delete_typo_tolerance(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let uid = path.into_inner();
    let mut s = match load_or_404(&state, &uid) {
        Ok(s) => s,
        Err(r) => return r,
    };
    s.typo_tolerance = TypoToleranceSettings::default();
    save(&state, &uid, s).await
}

// ---------------- distinct-attribute ----------------

/// Returns the distinct field.
#[utoipa::path(
    get,
    path = "/api/v1/collections/{uid}/settings/distinct-field",
    params(("uid" = String, Path,)),
    responses(
        (status = 200, description = "Distinct field", body = DistinctField),
        (status = 404, description = "Collection not found")
    ),
    tag = "collections"
)]
#[get("/collections/{uid}/settings/distinct-field")]
pub async fn get_distinct_field(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let uid = path.into_inner();
    match load_or_404(&state, &uid) {
        Ok(s) => HttpResponse::Ok().json(DistinctField {
            distinct_field: s.distinct_field,
        }),
        Err(r) => r,
    }
}

/// Sets the distinct field.
#[utoipa::path(
    put,
    path = "/api/v1/collections/{uid}/settings/distinct-field",
    params(("uid" = String, Path,)),
    request_body = DistinctField,
    responses(
        (status = 200, description = "Distinct field updated", body = DistinctField),
        (status = 404, description = "Collection not found")
    ),
    tag = "collections"
)]
#[put("/collections/{uid}/settings/distinct-field")]
pub async fn put_distinct_field(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<DistinctField>,
) -> HttpResponse {
    let uid = path.into_inner();
    let mut s = match load_or_404(&state, &uid) {
        Ok(s) => s,
        Err(r) => return r,
    };
    s.distinct_field = body.into_inner().distinct_field;
    save(&state, &uid, s).await
}

/// Removes the distinct field.
#[utoipa::path(
    delete,
    path = "/api/v1/collections/{uid}/settings/distinct-field",
    params(("uid" = String, Path,)),
    responses(
        (status = 200, description = "Distinct field removed", body = DistinctField),
        (status = 404, description = "Collection not found")
    ),
    tag = "collections"
)]
#[delete("/collections/{uid}/settings/distinct-field")]
pub async fn delete_distinct_field(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let uid = path.into_inner();
    let mut s = match load_or_404(&state, &uid) {
        Ok(s) => s,
        Err(r) => return r,
    };
    s.distinct_field = None;
    save(&state, &uid, s).await
}
