//! Tenant token endpoints.
//!
//! Tenant tokens are short-lived JWTs that downstream applications embed in
//! search-only API keys so they can run searches on behalf of an end user
//! without exposing the master key. Each token claims:
//!
//! * `apiKeyUid` – the API key used to mint the token
//! * `exp` – expiry timestamp
//! * `searchRules` – collection-level access restrictions

use actix_web::{HttpResponse, ResponseError, post, web};
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use models::Permission;
use serde::{Deserialize, Serialize};

use errors::AppError;
use models::{ApiKey, CollectionId};

use crate::state::AppState;

/// Request body for minting a tenant token.
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct TenantTokenRequest {
    /// Identifier of the API key to embed in the token.
    pub api_key_uid: String,
    /// Validity window in seconds (max 1 hour, default 1 hour).
    #[serde(default)]
    pub expires_after_seconds: Option<i64>,
    /// Optional search rules restricting what the token can search.
    #[serde(default)]
    pub search_rules: Option<Vec<SearchRule>>,
}

/// Search rule attached to a tenant token.
#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct SearchRule {
    /// Collection UID this rule applies to. `*` means all.
    pub index_uid: String,
    /// Optional additional search-time restrictions.
    #[serde(default)]
    pub filter: Option<String>,
}

/// Response body containing the encoded tenant token.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct TenantTokenResponse {
    /// Signed JWT.
    pub token: String,
    /// Expiry timestamp (UTC).
    pub expires_at: DateTime<Utc>,
}

const MAX_VALIDITY_SECONDS: i64 = 60 * 60;
const DEFAULT_VALIDITY_SECONDS: i64 = 60 * 60;

/// Mints a short-lived tenant token bound to an existing API key.
#[utoipa::path(
    post,
    path = "/api/v1/tenant-tokens",
    request_body = TenantTokenRequest,
    responses(
        (status = 201, description = "Token issued", body = TenantTokenResponse),
        (status = 400, description = "Invalid request"),
        (status = 404, description = "API key not found")
    ),
    tag = "tenant-tokens"
)]
#[post("/tenant-tokens")]
pub async fn generate_tenant_token(
    state: web::Data<AppState>,
    body: web::Json<TenantTokenRequest>,
) -> HttpResponse {
    let req = body.into_inner();
    let api_key_uid = match uuid::Uuid::parse_str(&req.api_key_uid) {
        Ok(u) => models::ApiKeyId::from_uuid(u),
        Err(_) => return AppError::bad_request("invalid api_key_uid").error_response(),
    };
    let key = match state.auth.get_key(api_key_uid).await {
        Ok(Some(k)) => k,
        Ok(None) => return AppError::not_found("API key not found").error_response(),
        Err(e) => return e.error_response(),
    };
    if !key_has_search(&key) {
        return AppError::forbidden("API key does not have search permission").error_response();
    }
    let validity = req
        .expires_after_seconds
        .unwrap_or(DEFAULT_VALIDITY_SECONDS)
        .clamp(1, MAX_VALIDITY_SECONDS);
    let exp = Utc::now() + Duration::seconds(validity);
    let rules = req.search_rules.unwrap_or_default();
    let claims = TenantClaims {
        api_key_uid: req.api_key_uid,
        exp: exp.timestamp(),
        search_rules: rules
            .into_iter()
            .map(|r| ClaimSearchRule {
                index_uid: r.index_uid,
                filter: r.filter,
            })
            .collect(),
    };
    let secret = state
        .config
        .auth
        .tenant_token_secret
        .clone()
        .unwrap_or_else(|| state.config.auth.master_key.clone());
    let token = match encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    ) {
        Ok(t) => t,
        Err(e) => return AppError::Internal(format!("jwt encode failed: {e}")).error_response(),
    };
    HttpResponse::Created().json(TenantTokenResponse {
        token,
        expires_at: exp,
    })
}

fn key_has_search(key: &ApiKey) -> bool {
    key.actions.contains(&Permission::All) || key.actions.contains(&Permission::Search)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TenantClaims {
    api_key_uid: String,
    exp: i64,
    search_rules: Vec<ClaimSearchRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClaimSearchRule {
    index_uid: String,
    filter: Option<String>,
}

/// Validates a tenant token against an optional `indexUid`. Used internally
/// by the request authentication pipeline.
pub fn validate_tenant_token(
    token: &str,
    secret: &[u8],
    index_uid: Option<&CollectionId>,
) -> Result<String, AppError> {
    use jsonwebtoken::{DecodingKey, Validation, decode};

    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    let data = decode::<TenantClaims>(token, &DecodingKey::from_secret(secret), &validation)
        .map_err(|e| AppError::Unauthorized(format!("invalid tenant token: {e}")))?;
    if let Some(uid) = index_uid {
        let allow = data
            .claims
            .search_rules
            .iter()
            .any(|r| r.index_uid == "*" || r.index_uid == uid.as_str());
        if !allow {
            return Err(AppError::Forbidden(
                "tenant token not authorized for this index".into(),
            ));
        }
    }
    Ok(data.claims.api_key_uid)
}
