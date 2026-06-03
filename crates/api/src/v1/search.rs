//! Search REST endpoints.

use ::search::dto::{SearchRequest, SearchResponse};
use actix_web::{HttpResponse, ResponseError, get, post, web};
use models::CollectionId;
use serde::Deserialize;

use accelerate_metrics::SEARCH_REQUESTS_TOTAL;
use errors::AppError;
use utils::Stopwatch;

use crate::state::AppState;

/// Executes a search.
#[utoipa::path(
    post,
    path = "/api/v1/collections/{uid}/search",
    params(("uid" = String, Path,)),
    request_body = SearchRequest,
    responses(
        (status = 200, description = "Search response", body = SearchResponse),
        (status = 404, description = "Collection not found")
    ),
    tag = "search"
)]
#[post("/collections/{uid}/search")]
pub async fn search(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<SearchRequest>,
) -> HttpResponse {
    let uid = CollectionId::new(path.into_inner());
    if state.collections.get(&uid).is_none() {
        return AppError::not_found(format!("collection '{uid}' not found")).error_response();
    }
    let sw = Stopwatch::new();
    let result = state.search.search(&uid, body.into_inner()).await;
    SEARCH_REQUESTS_TOTAL
        .with_label_values(&[uid.as_str()])
        .inc();
    accelerate_metrics::SEARCH_DURATION_SECONDS.observe(sw.elapsed().as_secs_f64());
    match result {
        Ok(r) => HttpResponse::Ok().json(r),
        Err(e) => e.error_response(),
    }
}

/// Executes a search via GET (for browser convenience).
#[utoipa::path(
    get,
    path = "/api/v1/collections/{uid}/search",
    params(
        ("uid" = String, Path,),
        ("q" = Option<String>, Query,),
        ("offset" = Option<usize>, Query,),
        ("limit" = Option<usize>, Query,)
    ),
    responses(
        (status = 200, description = "Search response", body = SearchResponse)
    ),
    tag = "search"
)]
#[get("/collections/{uid}/search")]
pub async fn search_get(
    state: web::Data<AppState>,
    path: web::Path<String>,
    query: web::Query<GetSearchQuery>,
) -> HttpResponse {
    let req = SearchRequest {
        q: query.q.clone(),
        offset: query.offset.unwrap_or(0),
        limit: query.limit,
        filter: query.filter.clone(),
        facets: query.facets.clone(),
        attributes_to_retrieve: None,
        attributes_to_highlight: None,
        sort: None,
        show_ranking_score: false,
        hybrid: None,
        vector: None,
        distinct: None,
        extra: Default::default(),
    };
    let uid = CollectionId::new(path.into_inner());
    if state.collections.get(&uid).is_none() {
        return AppError::not_found(format!("collection '{uid}' not found")).error_response();
    }
    match state.search.search(&uid, req).await {
        Ok(r) => HttpResponse::Ok().json(r),
        Err(e) => e.error_response(),
    }
}

/// GET-search query parameters.
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct GetSearchQuery {
    /// Query string.
    pub q: Option<String>,
    /// Page offset.
    pub offset: Option<usize>,
    /// Page limit.
    pub limit: Option<usize>,
    /// Filter expression.
    pub filter: Option<String>,
    /// Facet fields.
    pub facets: Option<Vec<String>>,
}

/// Multi-search request body.
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct MultiSearchRequest {
    /// Per-collection queries.
    pub queries: Vec<MultiSearchQuery>,
}

/// One query in a multi-search.
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct MultiSearchQuery {
    /// Collection UID.
    pub index_uid: String,
    /// Search request.
    #[serde(flatten)]
    pub request: SearchRequest,
}

/// Multi-search response.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct MultiSearchResponse {
    /// Per-collection results.
    pub results: Vec<MultiSearchResultEntry>,
}

/// Per-collection result entry.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct MultiSearchResultEntry {
    /// Collection UID.
    pub index_uid: String,
    /// Search response.
    #[serde(flatten)]
    pub response: SearchResponse,
}

/// Executes a search across multiple collections.
#[utoipa::path(
    post,
    path = "/api/v1/multi-search",
    request_body = MultiSearchRequest,
    responses(
        (status = 200, description = "Multi-search results", body = MultiSearchResponse)
    ),
    tag = "search"
)]
#[post("/multi-search")]
pub async fn multi_search(
    state: web::Data<AppState>,
    body: web::Json<MultiSearchRequest>,
) -> HttpResponse {
    let mut results = Vec::with_capacity(body.queries.len());
    for q in &body.queries {
        let uid = CollectionId::new(q.index_uid.clone());
        if state.collections.get(&uid).is_none() {
            results.push(MultiSearchResultEntry {
                index_uid: q.index_uid.clone(),
                response: SearchResponse {
                    query: q.request.q.clone(),
                    hits: Vec::new(),
                    estimated_total_hits: 0,
                    offset: q.request.offset,
                    limit: q.request.limit.unwrap_or(20),
                    processing_time_ms: 0,
                    facet_distribution: None,
                },
            });
            continue;
        }
        match state.search.search(&uid, q.request.clone()).await {
            Ok(r) => results.push(MultiSearchResultEntry {
                index_uid: q.index_uid.clone(),
                response: r,
            }),
            Err(_) => results.push(MultiSearchResultEntry {
                index_uid: q.index_uid.clone(),
                response: SearchResponse {
                    query: q.request.q.clone(),
                    hits: Vec::new(),
                    estimated_total_hits: 0,
                    offset: q.request.offset,
                    limit: q.request.limit.unwrap_or(20),
                    processing_time_ms: 0,
                    facet_distribution: None,
                },
            }),
        }
    }
    HttpResponse::Ok().json(MultiSearchResponse { results })
}
