//! Structured logging and tracing for AccelerateSearch.
//!
//! Wraps `tracing-subscriber` to provide:
//!
//! * `pretty` format (default in dev).
//! * `json` format (default in prod).
//! * File-based log appender with rotation.
//! * Configurable log level via config file, env var (`RUST_LOG`), or CLI.

use std::path::Path;

use config_crate::LoggingConfig;
use errors::{AppError, AppResult};
use tracing::Level;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Holds non-blocking writer guards for the logger. Keep the guard alive for
/// the lifetime of the process so logs are flushed correctly.
pub struct TelemetryGuard {
    _guard: Option<WorkerGuard>,
}

impl TelemetryGuard {
    /// Creates a no-op guard.
    #[must_use]
    pub fn disabled() -> Self {
        Self { _guard: None }
    }
}

/// Initialises the global tracing subscriber.
///
/// The function is idempotent — calling it more than once is a no-op.
///
/// # Errors
/// Returns an error if the log directory cannot be created.
pub fn init(cfg: &LoggingConfig) -> AppResult<TelemetryGuard> {
    init_with_service_name(cfg, "accelerate")
}

/// Initialises the global tracing subscriber with a custom service name.
pub fn init_with_service_name(
    cfg: &LoggingConfig,
    service_name: &str,
) -> AppResult<TelemetryGuard> {
    static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    let mut initialised = false;
    INIT.get_or_init(|| {
        initialised = true;
    });
    if !initialised {
        return Ok(TelemetryGuard::disabled());
    }

    let level = parse_level(&cfg.level);
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("accelerate={level},{service_name}={level}")));

    match cfg.format.as_str() {
        "json" => init_json(cfg, env_filter),
        _ => init_pretty(cfg, env_filter),
    }
}

fn init_pretty(cfg: &LoggingConfig, env_filter: EnvFilter) -> AppResult<TelemetryGuard> {
    let use_color = utils::color::stdout_is_colored() && !cfg.no_color;
    if cfg.no_console {
        if cfg.no_file {
            let registry = tracing_subscriber::registry().with(env_filter);
            registry
                .try_init()
                .map_err(|e| AppError::Internal(format!("logger: {e}")))?;
            return Ok(TelemetryGuard::disabled());
        }
        utils::ensure_dir(&cfg.dir).map_err(|e| {
            AppError::Internal(format!(
                "failed to create log dir {}: {e}",
                cfg.dir.display()
            ))
        })?;
        let file_appender =
            tracing_appender::rolling::daily(&cfg.dir, format!("-{}", cfg.file_prefix));
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
        let fmt_layer = tracing_subscriber::fmt::layer()
            .with_target(true)
            .with_thread_ids(false)
            .with_thread_names(false)
            .with_line_number(false)
            .with_file(false)
            .with_writer(non_blocking)
            .with_ansi(false);
        let registry = tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer);
        registry
            .try_init()
            .map_err(|e| AppError::Internal(format!("logger: {e}")))?;
        return Ok(TelemetryGuard {
            _guard: Some(guard),
        });
    }

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_line_number(false)
        .with_file(false)
        .with_ansi(use_color);

    if cfg.no_file {
        let registry = tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer);
        registry
            .try_init()
            .map_err(|e| AppError::Internal(format!("logger: {e}")))?;
        return Ok(TelemetryGuard::disabled());
    }

    utils::ensure_dir(&cfg.dir).map_err(|e| {
        AppError::Internal(format!(
            "failed to create log dir {}: {e}",
            cfg.dir.display()
        ))
    })?;
    let file_appender = tracing_appender::rolling::daily(&cfg.dir, format!("-{}", cfg.file_prefix));
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    let file_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_line_number(false)
        .with_file(false)
        .with_writer(non_blocking)
        .with_ansi(false);

    let registry = tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .with(file_layer);
    registry
        .try_init()
        .map_err(|e| AppError::Internal(format!("logger: {e}")))?;
    Ok(TelemetryGuard {
        _guard: Some(guard),
    })
}

fn init_json(cfg: &LoggingConfig, env_filter: EnvFilter) -> AppResult<TelemetryGuard> {
    utils::ensure_dir(&cfg.dir).map_err(|e| {
        AppError::Internal(format!(
            "failed to create log dir {}: {e}",
            cfg.dir.display()
        ))
    })?;
    let file_appender = tracing_appender::rolling::daily(&cfg.dir, format!("-{}", cfg.file_prefix));
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    let json_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_current_span(true)
        .with_span_list(false)
        .with_writer(non_blocking);

    let stdout_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_writer(std::io::stdout);

    let registry = tracing_subscriber::registry()
        .with(env_filter)
        .with(json_layer)
        .with(stdout_layer);
    registry
        .try_init()
        .map_err(|e| AppError::Internal(format!("logger: {e}")))?;
    Ok(TelemetryGuard {
        _guard: Some(guard),
    })
}

fn parse_level(s: &str) -> Level {
    match s.to_ascii_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" | "warning" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    }
}

/// Cleans up log files older than `days` in the given directory.
///
/// # Errors
/// Returns an I/O error if reading the directory fails.
pub fn cleanup_old_logs(dir: &Path, days: u64) -> std::io::Result<u64> {
    if !dir.exists() {
        return Ok(0);
    }
    let threshold = chrono::Duration::days(days as i64);
    let cutoff = chrono::Utc::now() - threshold;
    let mut removed = 0u64;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file()
            && let Ok(meta) = entry.metadata()
            && let Ok(modified) = meta.modified()
        {
            let modified_dt: chrono::DateTime<chrono::Utc> = modified.into();
            if modified_dt < cutoff && std::fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_level_works() {
        assert_eq!(parse_level("trace"), Level::TRACE);
        assert_eq!(parse_level("DEBUG"), Level::DEBUG);
        assert_eq!(parse_level("info"), Level::INFO);
        assert_eq!(parse_level("warn"), Level::WARN);
        assert_eq!(parse_level("error"), Level::ERROR);
        assert_eq!(parse_level("garbage"), Level::INFO);
    }

    #[test]
    fn cleanup_old_logs_no_dir_is_zero() {
        let tmp = std::env::temp_dir().join("accelerate-test-no-such-dir");
        let _ = std::fs::remove_dir_all(&tmp);
        let removed = cleanup_old_logs(&tmp, 30).unwrap();
        assert_eq!(removed, 0);
    }
}
