//! Cross-cutting security features for AccelerateSearch.
//!
//! Includes:
//!
//! * Rate limiting middleware (per IP and per principal) using `governor`.
//! * Security response headers.
//! * CORS middleware.
//! * Audit logging of authenticated write operations.

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::Arc;

use actix_service::Transform;
use actix_web::Error as ActixError;
use actix_web::body::EitherBody;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse};
use actix_web::http::Method;
use actix_web::http::header::{
    CONTENT_SECURITY_POLICY, HeaderName, HeaderValue, X_CONTENT_TYPE_OPTIONS, X_FRAME_OPTIONS,
    X_XSS_PROTECTION,
};
use chrono::Utc;
use futures_util::future::{LocalBoxFuture, Ready, ready};
use governor::clock::DefaultClock;
use governor::state::{InMemoryState, NotKeyed};
use governor::{Quota, RateLimiter};
use tracing::info;

use config_crate::{CorsConfig, RateLimitConfig};

/// Type alias for a per-key in-memory rate limiter.
type DirectLimiter = RateLimiter<NotKeyed, InMemoryState, DefaultClock>;

/// Wrapper around a rate limiter pool keyed by a string (IP or principal).
pub struct RateLimiterPool {
    inner: parking_lot::Mutex<HashMap<String, Arc<DirectLimiter>>>,
    quota: Quota,
    enabled: bool,
}

impl RateLimiterPool {
    /// Creates a new rate-limiter pool with the given configuration.
    #[must_use]
    pub fn new(cfg: &RateLimitConfig) -> Self {
        let rps = NonZeroU32::new(cfg.requests_per_second.max(1)).unwrap();
        let burst = NonZeroU32::new(cfg.burst_size.max(1)).unwrap();
        let quota = Quota::per_second(rps).allow_burst(burst);
        Self {
            inner: parking_lot::Mutex::new(HashMap::new()),
            quota,
            enabled: cfg.enabled,
        }
    }

    /// Returns true if the request should be allowed for the given key.
    #[must_use]
    pub fn check(&self, key: &str) -> bool {
        if !self.enabled {
            return true;
        }
        let mut guard = self.inner.lock();
        let limiter = guard
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(RateLimiter::direct(self.quota)))
            .clone();
        limiter.check().is_ok()
    }
}

/// Middleware that enforces rate limits based on the client's IP address.
#[derive(Clone)]
pub struct RateLimitMiddleware {
    pool: Arc<RateLimiterPool>,
}

impl RateLimitMiddleware {
    /// Creates a new rate-limit middleware.
    #[must_use]
    pub fn new(pool: Arc<RateLimiterPool>) -> Self {
        Self { pool }
    }
}

impl<S, B> Transform<S, ServiceRequest> for RateLimitMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = ActixError> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = ActixError;
    type Transform = RateLimitMiddlewareInner<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RateLimitMiddlewareInner {
            service,
            pool: self.pool.clone(),
        }))
    }
}

pub struct RateLimitMiddlewareInner<S> {
    service: S,
    pool: Arc<RateLimiterPool>,
}

impl<S, B> Service<ServiceRequest> for RateLimitMiddlewareInner<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = ActixError> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = ActixError;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    actix_web::dev::forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let ip = req
            .connection_info()
            .realip_remote_addr()
            .unwrap_or("unknown")
            .to_string();
        let allowed = self.pool.check(&ip);
        let fut = self.service.call(req);
        Box::pin(async move {
            if !allowed {
                let resp = actix_web::HttpResponse::TooManyRequests()
                    .insert_header(("Retry-After", "1"))
                    .json(serde_json::json!({
                        "error": "rate_limited",
                        "message": "Too many requests, please retry later.",
                        "code": 429
                    }));
                return Ok(ServiceResponse::new(
                    actix_web::test::TestRequest::default().to_http_request(),
                    resp,
                )
                .map_into_right_body());
            }
            let res = fut.await?;
            Ok(res.map_into_left_body())
        })
    }
}

/// Returns the default set of security headers applied to every response.
#[must_use]
pub fn security_headers() -> Vec<(HeaderName, HeaderValue)> {
    vec![
        (X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")),
        (X_FRAME_OPTIONS, HeaderValue::from_static("DENY")),
        (
            HeaderName::from_static("strict-transport-security"),
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        ),
        (
            CONTENT_SECURITY_POLICY,
            HeaderValue::from_static("default-src 'self'"),
        ),
        (X_XSS_PROTECTION, HeaderValue::from_static("1; mode=block")),
        (
            HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static("no-referrer"),
        ),
    ]
}

/// Middleware that injects the default security headers into every response.
#[derive(Clone, Default)]
pub struct SecurityHeadersMiddleware;

impl<S, B> Transform<S, ServiceRequest> for SecurityHeadersMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = ActixError> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = ActixError;
    type Transform = SecurityHeadersInner<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(SecurityHeadersInner { service }))
    }
}

pub struct SecurityHeadersInner<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for SecurityHeadersInner<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = ActixError> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = ActixError;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    actix_web::dev::forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let fut = self.service.call(req);
        Box::pin(async move {
            let mut res = fut.await?;
            for (name, value) in security_headers() {
                res.headers_mut().insert(name, value);
            }
            Ok(res)
        })
    }
}

/// Audit log entry.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AuditEntry {
    /// When the action happened (UTC).
    pub timestamp: chrono::DateTime<Utc>,
    /// Name of the principal performing the action.
    pub principal: String,
    /// HTTP method.
    pub method: String,
    /// HTTP path.
    pub path: String,
    /// Resource affected (e.g. collection UID).
    pub resource: Option<String>,
    /// Action label.
    pub action: String,
}

/// Audit logger. Logs structured entries via `tracing`.
pub struct AuditLog;

impl AuditLog {
    /// Writes an audit entry to the structured log.
    pub fn record(entry: AuditEntry) {
        info!(
            target: "audit",
            timestamp = %entry.timestamp,
            principal = %entry.principal,
            method = %entry.method,
            path = %entry.path,
            resource = entry.resource.as_deref().unwrap_or("-"),
            action = %entry.action,
            "audit"
        );
    }

    /// Convenience constructor for a write audit entry.
    pub fn write(
        principal: &str,
        method: &str,
        path: &str,
        resource: Option<String>,
        action: &str,
    ) {
        Self::record(AuditEntry {
            timestamp: Utc::now(),
            principal: principal.to_string(),
            method: method.to_string(),
            path: path.to_string(),
            resource,
            action: action.to_string(),
        });
    }
}

/// Sanitises a user-provided string by stripping control characters and
/// null bytes. Convenience re-export of [`utils::sanitize_string`].
#[must_use]
pub fn sanitize(s: &str) -> String {
    utils::sanitize_string(s)
}

/// CORS middleware configuration.
#[derive(Clone)]
pub struct CorsMiddleware {
    config: CorsConfig,
}

impl CorsMiddleware {
    /// Creates a new CORS middleware with the given configuration.
    #[must_use]
    pub fn new(config: CorsConfig) -> Self {
        Self { config }
    }
}

impl<S, B> Transform<S, ServiceRequest> for CorsMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = ActixError> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = ActixError;
    type Transform = CorsMiddlewareInner<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(CorsMiddlewareInner {
            service,
            config: self.config.clone(),
        }))
    }
}

pub struct CorsMiddlewareInner<S> {
    service: S,
    config: CorsConfig,
}

impl<S, B> Service<ServiceRequest> for CorsMiddlewareInner<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = ActixError> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = ActixError;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    actix_web::dev::forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        if !self.config.enabled {
            let fut = self.service.call(req);
            return Box::pin(fut);
        }

        let origin = req
            .headers()
            .get("Origin")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);

        let method = req.method().clone();

        let fut = self.service.call(req);
        let config = self.config.clone();

        Box::pin(async move {
            let mut res = fut.await?;

            if let Some(ref origin) = origin {
                let allowed = config.allowed_origins.is_empty()
                    || config.allowed_origins.iter().any(|o| o == origin);

                if allowed {
                    res.headers_mut().insert(
                        HeaderName::from_static("access-control-allow-origin"),
                        HeaderValue::from_str(origin)
                            .unwrap_or_else(|_| HeaderValue::from_static("*")),
                    );

                    if config.allow_credentials {
                        res.headers_mut().insert(
                            HeaderName::from_static("access-control-allow-credentials"),
                            HeaderValue::from_static("true"),
                        );
                    }

                    res.headers_mut().insert(
                        HeaderName::from_static("access-control-allow-methods"),
                        HeaderValue::from_str(&config.allowed_methods.join(", ")).unwrap_or_else(
                            |_| HeaderValue::from_static("GET, POST, PUT, PATCH, DELETE, OPTIONS"),
                        ),
                    );

                    res.headers_mut().insert(
                        HeaderName::from_static("access-control-allow-headers"),
                        HeaderValue::from_str(&config.allowed_headers.join(", ")).unwrap_or_else(
                            |_| HeaderValue::from_static("Authorization, Content-Type, Accept"),
                        ),
                    );

                    res.headers_mut().insert(
                        HeaderName::from_static("access-control-max-age"),
                        HeaderValue::from_str(&config.max_age.to_string())
                            .unwrap_or_else(|_| HeaderValue::from_static("3600")),
                    );
                }
            }

            // Handle preflight OPTIONS request
            if method == Method::OPTIONS {
                res.headers_mut().insert(
                    HeaderName::from_static("access-control-allow-origin"),
                    HeaderValue::from_static("*"),
                );
            }

            Ok(res)
        })
    }
}

/// Wait-time helper for the test suite.
#[cfg(test)]
pub fn _unused() {
    let _ = std::time::Duration::from_secs(1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limiter_disabled_allows_everything() {
        let cfg = RateLimitConfig {
            enabled: false,
            ..Default::default()
        };
        let pool = RateLimiterPool::new(&cfg);
        for _ in 0..10_000 {
            assert!(pool.check("1.2.3.4"));
        }
    }

    #[test]
    fn rate_limiter_enabled_eventually_throttles() {
        let cfg = RateLimitConfig {
            enabled: true,
            requests_per_second: 1,
            burst_size: 2,
        };
        let pool = RateLimiterPool::new(&cfg);
        assert!(pool.check("a"));
        assert!(pool.check("a"));
        // The third call within the same second should be denied.
        assert!(!pool.check("a"));
    }

    #[test]
    fn security_headers_returns_known_set() {
        let h = security_headers();
        assert!(!h.is_empty());
        let names: Vec<_> = h.iter().map(|(n, _)| n.as_str().to_string()).collect();
        assert!(names.iter().any(|n| n == "x-content-type-options"));
        assert!(names.iter().any(|n| n == "x-frame-options"));
        assert!(names.iter().any(|n| n == "content-security-policy"));
    }

    #[test]
    fn sanitize_strips_nulls() {
        let s = sanitize("hello\u{0}world");
        assert_eq!(s, "helloworld");
    }
}
