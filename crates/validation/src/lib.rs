//! Input validation for AccelerateSearch.
//!
//! Provides:
//!
//! * Validate collection identifiers (alphanumeric + `-` + `_`, max 512).
//! * Validate document field names.
//! * Validate document primary key types.
//! * Validate search queries and filter expressions.
//! * Reusable [`validator::Validate`] traits for the request DTOs.

use errors::{AppError, AppResult};
use once_cell::sync::Lazy;
use regex::Regex;

static COLLECTION_ID_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[a-zA-Z0-9_-]{1,512}$").expect("valid regex"));

static FIELD_NAME_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[a-zA-Z0-9_.]{1,512}$").expect("valid regex"));

static RESERVED_COLLECTION_IDS: &[&str] = &[
    "*", "all", "system", "internal", ".", "..", "metrics", "health", "version", "stats",
];

/// Maximum allowed length of a search query.
pub const MAX_QUERY_LENGTH: usize = 4096;

/// Maximum allowed length of a filter expression.
pub const MAX_FILTER_LENGTH: usize = 2048;

/// Maximum allowed length of a collection identifier.
pub const MAX_COLLECTION_ID_LENGTH: usize = 512;

/// Validates a collection identifier.
pub fn validate_collection_id(uid: &str) -> AppResult<()> {
    if uid.is_empty() {
        return Err(AppError::bad_request("collection uid is empty"));
    }
    if uid.len() > MAX_COLLECTION_ID_LENGTH {
        return Err(AppError::bad_request(format!(
            "collection uid exceeds {MAX_COLLECTION_ID_LENGTH} characters"
        )));
    }
    if !COLLECTION_ID_RE.is_match(uid) {
        return Err(AppError::bad_request(format!(
            "collection uid '{uid}' must contain only alphanumeric characters, hyphens, and underscores"
        )));
    }
    if RESERVED_COLLECTION_IDS.contains(&uid) {
        return Err(AppError::bad_request(format!(
            "collection uid '{uid}' is reserved"
        )));
    }
    Ok(())
}

/// Validates a document field name.
pub fn validate_field_name(name: &str) -> AppResult<()> {
    if name.is_empty() {
        return Err(AppError::bad_request("field name is empty"));
    }
    if name.len() > 512 {
        return Err(AppError::bad_request("field name too long"));
    }
    if !FIELD_NAME_RE.is_match(name) {
        return Err(AppError::bad_request(format!(
            "field name '{name}' must contain only alphanumeric characters, underscores, and dots"
        )));
    }
    Ok(())
}

/// Validates the primary key value of a document.
pub fn validate_primary_key(value: &serde_json::Value) -> AppResult<()> {
    match value {
        serde_json::Value::String(s) => {
            if s.is_empty() {
                return Err(AppError::bad_request("primary key string is empty"));
            }
            if s.len() > 512 {
                return Err(AppError::bad_request("primary key too long"));
            }
        }
        serde_json::Value::Number(n) => {
            if !(n.is_i64() || n.is_u64() || n.is_f64()) {
                return Err(AppError::bad_request("unsupported primary key number type"));
            }
        }
        _ => {
            return Err(AppError::bad_request(
                "primary key must be a string or number",
            ));
        }
    }
    Ok(())
}

/// Validates a search query.
pub fn validate_query(q: &str) -> AppResult<()> {
    if q.len() > MAX_QUERY_LENGTH {
        return Err(AppError::bad_request(format!(
            "query exceeds {MAX_QUERY_LENGTH} characters"
        )));
    }
    Ok(())
}

/// Strips control characters (`\0`-`\x1f` except `\n`, `\t`, `\r`) and
/// collapses runs of plain whitespace. Newlines and tabs are preserved.
/// Returns a `Cow<str>` to avoid an allocation when the input is
/// already clean.
#[must_use]
pub fn sanitize_input(s: &str) -> std::borrow::Cow<'_, str> {
    use std::borrow::Cow;
    let needs_clean = s
        .chars()
        .any(|c| c.is_control() && c != '\n' && c != '\t' && c != '\r')
        || s.contains("  ");
    if !needs_clean {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for c in s.chars() {
        if c.is_control() && c != '\n' && c != '\t' && c != '\r' {
            continue;
        }
        if c == '\n' || c == '\t' || c == '\r' {
            out.push(c);
            prev_space = false;
            continue;
        }
        if c.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(c);
            prev_space = false;
        }
    }
    Cow::Owned(out)
}

/// Validates a filter expression.
pub fn validate_filter(filter: &str) -> AppResult<()> {
    if filter.is_empty() {
        return Err(AppError::bad_request("filter is empty"));
    }
    if filter.len() > MAX_FILTER_LENGTH {
        return Err(AppError::bad_request(format!(
            "filter exceeds {MAX_FILTER_LENGTH} characters"
        )));
    }
    Ok(())
}

/// Validates an `ApiKeyCreate` payload.
pub fn validate_api_key_name(name: &str) -> AppResult<()> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::bad_request("api key name is empty"));
    }
    if trimmed.len() > 256 {
        return Err(AppError::bad_request("api key name too long"));
    }
    Ok(())
}

/// Validates a search pagination request.
pub fn validate_pagination(offset: usize, limit: usize, max_total_hits: usize) -> AppResult<()> {
    if limit == 0 {
        return Err(AppError::bad_request("limit must be > 0"));
    }
    if limit > max_total_hits {
        return Err(AppError::bad_request(format!(
            "limit must be <= {max_total_hits}"
        )));
    }
    if offset > max_total_hits {
        return Err(AppError::bad_request(format!(
            "offset must be <= {max_total_hits}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collection_id_accepts_alphanumeric_with_underscore_and_dash() {
        assert!(validate_collection_id("products").is_ok());
        assert!(validate_collection_id("my_collection-1").is_ok());
        assert!(validate_collection_id(&"x".repeat(512)).is_ok());
    }

    #[test]
    fn collection_id_rejects_invalid_characters() {
        assert!(validate_collection_id("").is_err());
        assert!(validate_collection_id("with space").is_err());
        assert!(validate_collection_id("a/b").is_err());
        assert!(validate_collection_id("a\\b").is_err());
    }

    #[test]
    fn collection_id_rejects_reserved() {
        assert!(validate_collection_id("*").is_err());
        assert!(validate_collection_id("system").is_err());
        assert!(validate_collection_id(".").is_err());
    }

    #[test]
    fn collection_id_rejects_too_long() {
        assert!(validate_collection_id(&"x".repeat(513)).is_err());
    }

    #[test]
    fn field_name_accepts_dots() {
        assert!(validate_field_name("user.name").is_ok());
        assert!(validate_field_name("price_usd").is_ok());
    }

    #[test]
    fn field_name_rejects_invalid() {
        assert!(validate_field_name("").is_err());
        assert!(validate_field_name("with space").is_err());
        assert!(validate_field_name("with/slash").is_err());
    }

    #[test]
    fn primary_key_accepts_string_and_number() {
        assert!(validate_primary_key(&serde_json::json!("abc")).is_ok());
        assert!(validate_primary_key(&serde_json::json!(42)).is_ok());
        assert!(validate_primary_key(&serde_json::json!(1.5)).is_ok());
    }

    #[test]
    fn primary_key_rejects_empty_string() {
        assert!(validate_primary_key(&serde_json::json!("")).is_err());
    }

    #[test]
    fn primary_key_rejects_other_types() {
        assert!(validate_primary_key(&serde_json::json!(true)).is_err());
        assert!(validate_primary_key(&serde_json::json!(null)).is_err());
        assert!(validate_primary_key(&serde_json::json!(["a"])).is_err());
    }

    #[test]
    fn query_validation() {
        assert!(validate_query("hello").is_ok());
        assert!(validate_query(&"x".repeat(MAX_QUERY_LENGTH + 1)).is_err());
    }

    #[test]
    fn filter_validation() {
        assert!(validate_filter("x = 1").is_ok());
        assert!(validate_filter("").is_err());
        assert!(validate_filter(&"x".repeat(MAX_FILTER_LENGTH + 1)).is_err());
    }

    #[test]
    fn pagination_validation() {
        assert!(validate_pagination(0, 20, 1000).is_ok());
        assert!(validate_pagination(0, 0, 1000).is_err());
        assert!(validate_pagination(0, 1001, 1000).is_err());
        assert!(validate_pagination(1001, 20, 1000).is_err());
    }

    #[test]
    fn api_key_name_validation() {
        assert!(validate_api_key_name("frontend-key").is_ok());
        assert!(validate_api_key_name("").is_err());
        assert!(validate_api_key_name(&"x".repeat(257)).is_err());
    }

    #[test]
    fn sanitize_input_strips_controls_and_collapses_whitespace() {
        use std::borrow::Cow;
        let r = sanitize_input("hello\0\x07 world  \nfoo");
        assert!(matches!(r, Cow::Owned(_)));
        // Newlines are preserved; runs of internal whitespace collapse.
        assert_eq!(r, "hello world \nfoo");
        let r = sanitize_input("clean");
        assert!(matches!(r, Cow::Borrowed(_)));
        assert_eq!(r, "clean");
    }
}
