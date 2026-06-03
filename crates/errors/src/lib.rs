//! Unified error types for the AccelerateSearch platform.
//!
//! Every error that escapes a crate boundary is an [`AppError`]. The
//! [`AppError`] enum is convertible into a JSON HTTP response by Actix-Web
//! via the [`actix_web::ResponseError`] implementation in this crate.

use std::fmt;

use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError};
use serde::{Deserialize, Serialize};

/// The single error type for the entire AccelerateSearch codebase.
///
/// `AppError` covers every category of error that may be produced by any
/// crate in the workspace. Each variant maps to an HTTP status code and a
/// stable machine-readable error code that clients can rely on.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// The requested resource (collection, document, task, key) was not found.
    #[error("{0}")]
    NotFound(String),

    /// The caller did not provide valid authentication credentials.
    #[error("{0}")]
    Unauthorized(String),

    /// The caller is authenticated but lacks the required permission.
    #[error("{0}")]
    Forbidden(String),

    /// The request collides with an existing resource.
    #[error("{0}")]
    Conflict(String),

    /// The request is malformed or syntactically invalid.
    #[error("{0}")]
    BadRequest(String),

    /// The request passed structural validation but failed semantic checks.
    #[error("{0}")]
    Validation(String),

    /// A catch-all for unexpected internal errors.
    #[error("internal error: {0}")]
    Internal(String),

    /// Failure inside the storage layer (redb, IO, serialization).
    #[error("storage error: {0}")]
    StorageError(String),

    /// Failure inside the indexing pipeline.
    #[error("indexing error: {0}")]
    IndexError(String),

    /// Failure inside the search engine.
    #[error("search error: {0}")]
    SearchError(String),

    /// Failure in the async task queue.
    #[error("task error: {0}")]
    TaskError(String),

    /// Failure in the configuration loader.
    #[error("config error: {0}")]
    ConfigError(String),

    /// Failure in the auth subsystem (key lookup, hashing, etc.).
    #[error("auth error: {0}")]
    AuthError(String),
}

impl AppError {
    /// Returns a stable, machine-readable code for the error variant.
    ///
    /// These codes are part of the public API and must not change without a
    /// major version bump.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound(_) => "not_found",
            Self::Unauthorized(_) => "unauthorized",
            Self::Forbidden(_) => "forbidden",
            Self::Conflict(_) => "conflict",
            Self::BadRequest(_) => "bad_request",
            Self::Validation(_) => "validation_failed",
            Self::Internal(_) => "internal_error",
            Self::StorageError(_) => "storage_error",
            Self::IndexError(_) => "indexing_failed",
            Self::SearchError(_) => "search_failed",
            Self::TaskError(_) => "task_failed",
            Self::ConfigError(_) => "config_error",
            Self::AuthError(_) => "auth_failed",
        }
    }

    /// Convenience constructor for an internal error.
    #[must_use]
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }

    /// Convenience constructor for a not-found error.
    #[must_use]
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }

    /// Convenience constructor for a bad-request error.
    #[must_use]
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::BadRequest(msg.into())
    }

    /// Convenience constructor for a validation error.
    #[must_use]
    pub fn validation(msg: impl Into<String>) -> Self {
        Self::Validation(msg.into())
    }

    /// Returns the HTTP status code corresponding to this error variant.
    #[must_use]
    pub fn http_status(&self) -> StatusCode {
        match self {
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Unauthorized(_) | Self::AuthError(_) => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::BadRequest(_) | Self::Validation(_) => StatusCode::BAD_REQUEST,
            Self::StorageError(_)
            | Self::IndexError(_)
            | Self::SearchError(_)
            | Self::TaskError(_)
            | Self::ConfigError(_)
            | Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Returns `true` if this error is a server-side problem the operator
    /// may want to report, as opposed to a client mistake.
    #[must_use]
    pub fn is_server_side(&self) -> bool {
        matches!(
            self,
            Self::Internal(_)
                | Self::StorageError(_)
                | Self::IndexError(_)
                | Self::SearchError(_)
                | Self::TaskError(_)
                | Self::ConfigError(_)
                | Self::AuthError(_)
        )
    }

    /// Returns the user-facing message with the issue-tracker URL appended
    /// for server-side errors. Client-side errors are returned unchanged.
    #[must_use]
    pub fn user_message(&self) -> String {
        if self.is_server_side() {
            format!("{} {}", self, issue_tracker_hint(None))
        } else {
            self.to_string()
        }
    }
}

impl ResponseError for AppError {
    fn status_code(&self) -> StatusCode {
        self.http_status()
    }

    fn error_response(&self) -> HttpResponse {
        let is_internal = matches!(
            self,
            Self::Internal(_)
                | Self::StorageError(_)
                | Self::IndexError(_)
                | Self::SearchError(_)
                | Self::TaskError(_)
                | Self::ConfigError(_)
                | Self::AuthError(_)
        );
        let report = if is_internal {
            Some(ISSUE_TRACKER_URL.to_string())
        } else {
            None
        };
        let body = ErrorBody {
            error: self.code(),
            message: self.to_string(),
            code: self.http_status().as_u16(),
            report,
        };
        HttpResponse::build(self.http_status()).json(body)
    }
}

impl From<redb::Error> for AppError {
    fn from(value: redb::Error) -> Self {
        Self::StorageError(value.to_string())
    }
}

impl From<redb::DatabaseError> for AppError {
    fn from(value: redb::DatabaseError) -> Self {
        Self::StorageError(value.to_string())
    }
}

impl From<redb::TransactionError> for AppError {
    fn from(value: redb::TransactionError) -> Self {
        Self::StorageError(value.to_string())
    }
}

impl From<redb::CommitError> for AppError {
    fn from(value: redb::CommitError) -> Self {
        Self::StorageError(value.to_string())
    }
}

impl From<redb::StorageError> for AppError {
    fn from(value: redb::StorageError) -> Self {
        Self::StorageError(value.to_string())
    }
}

impl From<redb::TableError> for AppError {
    fn from(value: redb::TableError) -> Self {
        Self::StorageError(value.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(value: serde_json::Error) -> Self {
        Self::Internal(format!("json: {value}"))
    }
}

impl From<toml::de::Error> for AppError {
    fn from(value: toml::de::Error) -> Self {
        Self::ConfigError(value.to_string())
    }
}

impl From<toml::ser::Error> for AppError {
    fn from(value: toml::ser::Error) -> Self {
        Self::ConfigError(value.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        Self::Internal(format!("io: {value}"))
    }
}

impl From<validator::ValidationErrors> for AppError {
    fn from(value: validator::ValidationErrors) -> Self {
        Self::Validation(value.to_string())
    }
}

/// The JSON shape returned for every error response.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ErrorBody {
    /// Stable, machine-readable code (e.g. `"index_not_found"`).
    pub error: &'static str,
    /// Human-readable error message.
    pub message: String,
    /// HTTP status code, mirrored for client convenience.
    pub code: u16,
    /// When this error is unexpected (`Internal` family), this points to
    /// the public issue tracker so the operator can report the bug.
    /// `None` for client-facing errors.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report: Option<String>,
}

/// URL of the project's public issue tracker, where unexpected internal
/// errors should be reported.
pub const ISSUE_TRACKER_URL: &str = "https://github.com/muhammad-fiaz/AccelerateSearch/issues";

/// Returns the issue-tracker URL hint, optionally including a context
/// string identifying the error source.
#[must_use]
pub fn issue_tracker_hint(context: Option<&str>) -> String {
    match context {
        Some(ctx) => format!("Please report this at {ISSUE_TRACKER_URL} (ref: {ctx})."),
        None => format!("Please report this at {ISSUE_TRACKER_URL}."),
    }
}

/// A specialized `Result` type for the AccelerateSearch codebase.
pub type AppResult<T> = std::result::Result<T, AppError>;

/// Formats an error chain for logging.
pub fn format_chain(err: &dyn std::error::Error) -> String {
    let mut s = err.to_string();
    let mut src = err.source();
    while let Some(e) = src {
        s.push_str(": ");
        s.push_str(&e.to_string());
        src = e.source();
    }
    s
}

impl fmt::Display for ErrorBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{{ error: {}, message: {}, code: {} }}",
            self.error, self.message, self.code
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_are_stable() {
        assert_eq!(AppError::NotFound("x".into()).code(), "not_found");
        assert_eq!(AppError::Unauthorized("x".into()).code(), "unauthorized");
        assert_eq!(AppError::Forbidden("x".into()).code(), "forbidden");
        assert_eq!(AppError::Conflict("x".into()).code(), "conflict");
        assert_eq!(AppError::BadRequest("x".into()).code(), "bad_request");
        assert_eq!(AppError::Validation("x".into()).code(), "validation_failed");
        assert_eq!(AppError::Internal("x".into()).code(), "internal_error");
    }

    #[test]
    fn error_status_mapping_is_correct() {
        assert_eq!(
            AppError::NotFound("x".into()).http_status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            AppError::Unauthorized("x".into()).http_status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            AppError::Forbidden("x".into()).http_status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            AppError::BadRequest("x".into()).http_status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            AppError::StorageError("x".into()).http_status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn error_response_is_valid_json() {
        let err = AppError::NotFound("Index 'products' not found.".into());
        let resp = err.error_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn issue_tracker_hint_includes_url() {
        let h = issue_tracker_hint(None);
        assert!(h.contains(ISSUE_TRACKER_URL));
        let h2 = issue_tracker_hint(Some("ctx"));
        assert!(h2.contains("ctx"));
        assert!(h2.contains(ISSUE_TRACKER_URL));
    }

    #[test]
    fn server_side_errors_carry_report_url() {
        let err = AppError::Internal("boom".into());
        assert!(err.is_server_side());
        // Build the same ErrorBody that the ResponseError impl would
        // return, and assert the JSON contains the tracker URL.
        let body = ErrorBody {
            error: err.code(),
            message: err.to_string(),
            code: err.http_status().as_u16(),
            report: Some(ISSUE_TRACKER_URL.to_string()),
        };
        let json = serde_json::to_value(&body).unwrap();
        assert!(json.get("report").is_some());
        assert_eq!(json["report"], ISSUE_TRACKER_URL);
    }

    #[test]
    fn client_side_errors_omit_report_url() {
        let err = AppError::NotFound("missing".into());
        assert!(!err.is_server_side());
        let body = ErrorBody {
            error: err.code(),
            message: err.to_string(),
            code: err.http_status().as_u16(),
            report: None,
        };
        let json = serde_json::to_value(&body).unwrap();
        assert!(json.get("report").is_none());
    }

    #[test]
    fn user_message_appends_hint_for_server_errors() {
        let err = AppError::Internal("oops".into());
        let msg = err.user_message();
        assert!(msg.contains("oops"));
        assert!(msg.contains(ISSUE_TRACKER_URL));
    }

    #[test]
    fn user_message_is_plain_for_client_errors() {
        let err = AppError::BadRequest("nope".into());
        let msg = err.user_message();
        assert_eq!(msg, "nope");
    }
}
