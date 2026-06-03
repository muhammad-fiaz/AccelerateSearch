//! Configuration system for AccelerateSearch.
//!
//! Loads a TOML file (with environment variable and CLI overrides), exposes
//! a strongly-typed [`AppConfig`] struct, and validates the result.

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

use errors::{AppError, AppResult};

/// Default config file path used when no override is supplied.
pub const DEFAULT_CONFIG_PATH: &str = "config/default.toml";

/// Top-level AccelerateSearch configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AppConfig {
    /// Server-related settings.
    pub server: ServerConfig,
    /// TLS/HTTPS configuration.
    pub tls: TlsConfig,
    /// Data directory and environment.
    pub data: DataConfig,
    /// Authentication configuration.
    pub auth: AuthConfig,
    /// Search engine configuration.
    pub search: SearchConfig,
    /// Indexing pipeline configuration.
    pub indexing: IndexingConfig,
    /// Vector search configuration.
    pub vector: VectorConfig,
    /// Logging configuration.
    pub logging: LoggingConfig,
    /// Metrics configuration.
    pub metrics: MetricsConfig,
    /// Snapshot configuration.
    pub snapshots: SnapshotsConfig,
    /// Update checker configuration.
    pub updates: UpdatesConfig,
    /// Rate limiting configuration.
    pub rate_limit: RateLimitConfig,
    /// Telemetry configuration.
    pub telemetry: TelemetryConfig,
    /// Cache configuration.
    pub cache: CacheConfig,
    /// CORS configuration.
    pub cors: CorsConfig,
    /// API documentation configuration.
    pub api_docs: ApiDocsConfig,
}

impl AppConfig {
    /// Loads the configuration from the given path.
    ///
    /// # Errors
    /// Returns an error if the file cannot be read or parsed.
    pub fn load(path: impl AsRef<std::path::Path>) -> AppResult<Self> {
        let content = std::fs::read_to_string(path.as_ref()).map_err(|e| {
            AppError::ConfigError(format!(
                "failed to read config file {}: {e}",
                path.as_ref().display()
            ))
        })?;
        let cfg: AppConfig = toml::from_str(&content)?;
        Ok(cfg)
    }

    /// Merges CLI overrides into the configuration.
    pub fn merge_cli(&mut self, cli: &CliOverrides) {
        if let Some(host) = &cli.host {
            self.server.host = host.clone();
        }
        if let Some(port) = cli.port {
            self.server.port = port;
        }
        if let Some(data_dir) = &cli.data_dir {
            self.data.dir = data_dir.clone();
        }
        if let Some(master_key) = &cli.master_key {
            self.auth.master_key = master_key.clone();
        }
        if let Some(level) = &cli.log_level {
            self.logging.level = level.clone();
        }
        if let Some(env) = &cli.env {
            self.data.env = env.clone();
        }
        if cli.disable_auth {
            self.auth.disable_auth = true;
        }
        if cli.no_color {
            self.logging.no_color = true;
        }
        if cli.quiet {
            self.logging.quiet = true;
        }
        if cli.no_console {
            self.logging.no_console = true;
        }
        if cli.no_file {
            self.logging.no_file = true;
        }
        if cli.no_swagger {
            self.api_docs.swagger_ui_enabled = false;
        }
        if cli.no_openapi {
            self.api_docs.openapi_enabled = false;
        }
    }

    /// Validates the configuration.
    ///
    /// # Errors
    /// Returns an error if any required value is missing or invalid.
    pub fn validate(&self) -> AppResult<()> {
        if self.server.port == 0 {
            return Err(AppError::ConfigError("server.port must be > 0".into()));
        }
        if self.data.env != "development" && self.data.env != "production" {
            return Err(AppError::ConfigError(format!(
                "data.env must be 'development' or 'production', got '{}'",
                self.data.env
            )));
        }
        if self.data.env == "production"
            && !self.auth.disable_auth
            && self.auth.master_key.is_empty()
        {
            return Err(AppError::ConfigError(
                "auth.master_key is required in production (or set auth.disable_auth = true)"
                    .into(),
            ));
        }
        if self.indexing.max_batch_size == 0 {
            return Err(AppError::ConfigError(
                "indexing.max_batch_size must be > 0".into(),
            ));
        }
        if self.vector.enabled && self.vector.dimensions == 0 {
            return Err(AppError::ConfigError(
                "vector.dimensions must be > 0 when vector.enabled = true".into(),
            ));
        }
        Ok(())
    }

    /// Returns the effective master key.
    #[must_use]
    pub fn effective_master_key(&self) -> &str {
        if self.auth.disable_auth {
            ""
        } else {
            &self.auth.master_key
        }
    }

    /// Returns true if authentication is enabled in this configuration.
    #[must_use]
    pub fn is_auth_enabled(&self) -> bool {
        !self.auth.disable_auth && !self.auth.master_key.is_empty()
    }
}

/// Server settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Bind address.
    pub host: String,
    /// Bind port.
    pub port: u16,
    /// Number of worker threads (`0` means auto = num CPUs).
    pub workers: usize,
    /// Maximum simultaneous connections.
    pub max_connections: usize,
    /// HTTP keep-alive timeout.
    pub keep_alive: String,
    /// Read timeout.
    pub read_timeout: String,
    /// Write timeout.
    pub write_timeout: String,
    /// Graceful shutdown timeout.
    pub shutdown_timeout: String,
    /// Maximum request body size in bytes.
    pub max_body_size: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "localhost".into(),
            port: 7700,
            workers: 0,
            max_connections: 0,
            keep_alive: "75s".into(),
            read_timeout: "30s".into(),
            write_timeout: "30s".into(),
            shutdown_timeout: "10s".into(),
            max_body_size: 100 * 1024 * 1024,
        }
    }
}

/// Data directory and environment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataConfig {
    /// Data directory.
    pub dir: PathBuf,
    /// Environment: `"development"` or `"production"`.
    pub env: String,
}

impl Default for DataConfig {
    fn default() -> Self {
        Self {
            dir: PathBuf::from("./data"),
            env: "development".into(),
        }
    }
}

/// Authentication configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Master key (a random hex string).
    pub master_key: String,
    /// If true, disable authentication entirely (dev mode only).
    pub disable_auth: bool,
    /// Optional secret used to sign and validate tenant tokens. If not
    /// configured, the master key (or a process-wide default) is used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_token_secret: Option<String>,
}

/// Search engine configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    /// Maximum number of facet values per field.
    pub max_values_per_facet: usize,
    /// Maximum total hits returned in a paginated response.
    pub pagination_max_total_hits: usize,
    /// BM25 `k1` parameter.
    pub bm25_k1: f64,
    /// BM25 `b` parameter.
    pub bm25_b: f64,
    /// Default page size.
    pub default_limit: usize,
    /// Maximum page size.
    pub max_limit: usize,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            max_values_per_facet: 100,
            pagination_max_total_hits: 1000,
            bm25_k1: 1.2,
            bm25_b: 0.75,
            default_limit: 20,
            max_limit: 1000,
        }
    }
}

/// Indexing pipeline configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingConfig {
    /// Maximum documents per batch.
    pub max_batch_size: usize,
    /// Commit interval in milliseconds.
    pub commit_interval_ms: u64,
    /// Maximum number of parallel indexing workers.
    pub parallelism: usize,
    /// Whether to apply stemming.
    pub stem: bool,
    /// Whether to remove stop words.
    pub remove_stop_words: bool,
}

impl Default for IndexingConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 1000,
            commit_interval_ms: 500,
            parallelism: 0,
            stem: true,
            remove_stop_words: true,
        }
    }
}

/// Vector search configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorConfig {
    /// Whether vector search is enabled.
    pub enabled: bool,
    /// Default embedding dimensions.
    pub dimensions: usize,
    /// Default similarity metric.
    pub similarity: String,
    /// HNSW `M` parameter.
    pub hnsw_m: usize,
    /// HNSW `ef_construction` parameter.
    pub hnsw_ef_construction: usize,
    /// HNSW `ef_search` parameter.
    pub hnsw_ef_search: usize,
    /// Quantization strategy: none | scalar | product | binary.
    pub quantization: String,
    /// Number of sub-spaces for product quantization.
    pub pq_m: usize,
    /// Number of centroids per sub-space for product quantization.
    pub pq_k: usize,
    /// Allow sparse vector embeddings (SPLADE-style).
    pub allow_sparse: bool,
    /// Allow multi-vector embeddings (ColBERT-style).
    pub allow_multi: bool,
    /// Optional external embedder URL for automatic embedding on ingest.
    pub embedder_url: Option<String>,
    /// Optional embedder model name reported in telemetry.
    pub embedder_model: Option<String>,
}

impl Default for VectorConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            dimensions: 384,
            similarity: "cosine".into(),
            hnsw_m: 16,
            hnsw_ef_construction: 200,
            hnsw_ef_search: 50,
            quantization: "none".into(),
            pq_m: 8,
            pq_k: 256,
            allow_sparse: true,
            allow_multi: true,
            embedder_url: None,
            embedder_model: None,
        }
    }
}

/// Logging configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level: trace | debug | info | warn | error.
    pub level: String,
    /// Log format: pretty | json.
    pub format: String,
    /// Log directory.
    pub dir: PathBuf,
    /// Log file prefix.
    pub file_prefix: String,
    /// Maximum number of retained log files.
    pub max_files: usize,
    /// Maximum size of a single log file in megabytes.
    pub max_size_mb: usize,
    /// Auto-delete logs older than N days.
    pub auto_delete_days: u64,
    /// Disable all console log output.
    pub no_console: bool,
    /// Disable the file log appender entirely.
    pub no_file: bool,
    /// Suppress coloured output in the console.
    pub no_color: bool,
    /// Silence all non-error output.
    pub quiet: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".into(),
            format: "pretty".into(),
            dir: PathBuf::from("./logs"),
            file_prefix: "accelerate".into(),
            max_files: 7,
            max_size_mb: 100,
            auto_delete_days: 30,
            no_console: false,
            no_file: false,
            no_color: false,
            quiet: false,
        }
    }
}

/// Metrics configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Whether metrics are exposed.
    pub enabled: bool,
    /// HTTP endpoint for the metrics scrape.
    pub endpoint: String,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            endpoint: "/metrics".into(),
        }
    }
}

/// Snapshots configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotsConfig {
    /// Snapshots directory.
    pub dir: PathBuf,
    /// Cron schedule for automatic snapshots.
    pub schedule: String,
    /// Whether automatic snapshots are enabled.
    pub auto_create: bool,
}

impl Default for SnapshotsConfig {
    fn default() -> Self {
        Self {
            dir: PathBuf::from("./snapshots"),
            schedule: "0 0 * * *".into(),
            auto_create: false,
        }
    }
}

/// Update checker configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatesConfig {
    /// Whether to check for new versions on startup.
    pub check_enabled: bool,
    /// Interval between checks.
    pub check_interval: String,
}

impl Default for UpdatesConfig {
    fn default() -> Self {
        Self {
            check_enabled: true,
            check_interval: "24h".into(),
        }
    }
}

/// Rate limiting configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Whether rate limiting is enabled.
    pub enabled: bool,
    /// Permitted requests per second.
    pub requests_per_second: u32,
    /// Allowed burst size.
    pub burst_size: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            requests_per_second: 100,
            burst_size: 200,
        }
    }
}

/// Telemetry configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConfig {
    /// Whether tracing is enabled.
    pub tracing_enabled: bool,
    /// Service name for traces.
    pub service_name: String,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            tracing_enabled: true,
            service_name: "accelerate".into(),
        }
    }
}

/// Cache configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Whether caching is enabled.
    pub enabled: bool,
    /// Maximum number of cache entries.
    pub max_entries: usize,
    /// Cache TTL in seconds.
    pub ttl_seconds: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_entries: 10_000,
            ttl_seconds: 300,
        }
    }
}

/// TLS/HTTPS configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TlsConfig {
    /// Enable HTTPS/TLS support.
    pub enabled: bool,
    /// Path to the TLS certificate file (PEM format).
    pub cert_path: String,
    /// Path to the TLS private key file (PEM format).
    pub key_path: String,
    /// Path to the CA certificate bundle for client certificate verification.
    pub ca_cert_path: String,
    /// Require client certificates (mutual TLS).
    pub require_client_cert: bool,
}

/// CORS configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsConfig {
    /// Whether CORS is enabled.
    pub enabled: bool,
    /// Allowed origins (empty = all).
    #[serde(default)]
    pub allowed_origins: Vec<String>,
    /// Allowed HTTP methods.
    #[serde(default = "default_cors_methods")]
    pub allowed_methods: Vec<String>,
    /// Allowed headers.
    #[serde(default = "default_cors_headers")]
    pub allowed_headers: Vec<String>,
    /// Allow credentials (cookies, authorization headers).
    pub allow_credentials: bool,
    /// Max age for preflight cache in seconds.
    pub max_age: u64,
}

fn default_cors_methods() -> Vec<String> {
    vec![
        "GET".into(),
        "POST".into(),
        "PUT".into(),
        "PATCH".into(),
        "DELETE".into(),
        "OPTIONS".into(),
    ]
}

fn default_cors_headers() -> Vec<String> {
    vec![
        "Authorization".into(),
        "Content-Type".into(),
        "Accept".into(),
        "Origin".into(),
        "X-Requested-With".into(),
    ]
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            allowed_origins: Vec::new(),
            allowed_methods: default_cors_methods(),
            allowed_headers: default_cors_headers(),
            allow_credentials: true,
            max_age: 3600,
        }
    }
}

/// API documentation configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiDocsConfig {
    /// Enable the Swagger UI at /swagger-ui.
    pub swagger_ui_enabled: bool,
    /// Enable the OpenAPI spec at /api-docs/openapi.json.
    pub openapi_enabled: bool,
}

impl Default for ApiDocsConfig {
    fn default() -> Self {
        Self {
            swagger_ui_enabled: true,
            openapi_enabled: true,
        }
    }
}

/// The `accelerate` command-line interface.
#[derive(Debug, Clone, Parser)]
#[command(
    name = "accelerate",
    bin_name = "accelerate",
    version,
    about = "AccelerateSearch - a modern self-hosted search engine",
    long_about = None
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<CliCommand>,
}

/// Top-level CLI subcommands.
#[derive(Debug, Clone, Subcommand)]
pub enum CliCommand {
    /// Start the AccelerateSearch server.
    Start {
        /// Path to the configuration file.
        #[arg(long, short = 'c', env = "ACCELERATE_CONFIG")]
        config: Option<PathBuf>,
        /// Bind host (overrides `[server] host`).
        #[arg(long, env = "ACCELERATE_HOST")]
        host: Option<String>,
        /// Bind port (overrides `[server] port`).
        #[arg(long, short = 'p', env = "ACCELERATE_PORT")]
        port: Option<u16>,
        /// Data directory (overrides `[data] dir`).
        #[arg(long, env = "ACCELERATE_DATA_DIR")]
        data_dir: Option<PathBuf>,
        /// Master API key (overrides `[auth] master_key`).
        #[arg(long, env = "ACCELERATE_MASTER_KEY")]
        master_key: Option<String>,
        /// Log level (overrides `[logging] level`).
        #[arg(long, env = "ACCELERATE_LOG_LEVEL")]
        log_level: Option<String>,
        /// Environment: development | production.
        #[arg(long, env = "ACCELERATE_ENV")]
        env: Option<String>,
        /// Disable authentication entirely (dev mode only).
        #[arg(long)]
        disable_auth: bool,
        /// Disable coloured console output.
        #[arg(long, env = "ACCELERATE_NO_COLOR")]
        no_color: bool,
        /// Silence all non-error output.
        #[arg(long, short = 'q', env = "ACCELERATE_QUIET")]
        quiet: bool,
        /// Disable the banner shown at startup.
        #[arg(long, env = "ACCELERATE_NO_BANNER")]
        no_banner: bool,
        /// Disable console log output entirely.
        #[arg(long, env = "ACCELERATE_NO_CONSOLE")]
        no_console: bool,
        /// Disable the file log appender entirely.
        #[arg(long, env = "ACCELERATE_NO_FILE")]
        no_file: bool,
        /// Disable the Swagger UI at /swagger-ui.
        #[arg(long, env = "ACCELERATE_NO_SWAGGER")]
        no_swagger: bool,
        /// Disable the OpenAPI spec at /api-docs/openapi.json.
        #[arg(long, env = "ACCELERATE_NO_OPENAPI")]
        no_openapi: bool,
    },
    /// Print version information and exit.
    Version,
    /// Run a health check against a running server.
    Health {
        /// Base URL of the running server.
        #[arg(long, default_value = "http://127.0.0.1:7700")]
        url: String,
    },
    /// Snapshot subcommands.
    Snapshot {
        #[command(subcommand)]
        action: SnapshotAction,
    },
}

/// Snapshot subcommands.
#[derive(Debug, Clone, Subcommand)]
pub enum SnapshotAction {
    /// Create a new snapshot of the running server's data.
    Create {
        /// Base URL of the running server.
        #[arg(long, default_value = "http://127.0.0.1:7700")]
        url: String,
        /// Optional master key.
        #[arg(long, env = "ACCELERATE_MASTER_KEY")]
        master_key: Option<String>,
    },
    /// List all snapshots on the running server.
    List {
        /// Base URL of the running server.
        #[arg(long, default_value = "http://127.0.0.1:7700")]
        url: String,
        /// Optional master key.
        #[arg(long, env = "ACCELERATE_MASTER_KEY")]
        master_key: Option<String>,
    },
    /// Print metadata of a single snapshot.
    Info {
        /// Snapshot name.
        name: String,
        /// Base URL of the running server.
        #[arg(long, default_value = "http://127.0.0.1:7700")]
        url: String,
        /// Optional master key.
        #[arg(long, env = "ACCELERATE_MASTER_KEY")]
        master_key: Option<String>,
    },
    /// Delete a snapshot from the running server.
    Delete {
        /// Snapshot name.
        name: String,
        /// Base URL of the running server.
        #[arg(long, default_value = "http://127.0.0.1:7700")]
        url: String,
        /// Optional master key.
        #[arg(long, env = "ACCELERATE_MASTER_KEY")]
        master_key: Option<String>,
    },
    /// Restore a snapshot from disk into a data directory.
    Restore {
        /// Path to the snapshot file.
        #[arg(long)]
        path: PathBuf,
        /// Data directory to restore into.
        #[arg(long, default_value = "./data")]
        data_dir: PathBuf,
    },
}

/// CLI values that override TOML configuration.
#[derive(Debug, Clone, Default)]
pub struct CliOverrides {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub data_dir: Option<PathBuf>,
    pub master_key: Option<String>,
    pub log_level: Option<String>,
    pub env: Option<String>,
    pub disable_auth: bool,
    pub no_color: bool,
    pub quiet: bool,
    pub no_banner: bool,
    pub no_console: bool,
    pub no_file: bool,
    pub no_swagger: bool,
    pub no_openapi: bool,
}

impl CliOverrides {
    /// Builds overrides from parsed CLI arguments.
    #[must_use]
    pub fn from_cli(cli: &Cli) -> Option<Self> {
        if let Some(CliCommand::Start {
            host,
            port,
            data_dir,
            master_key,
            log_level,
            env,
            disable_auth,
            no_color,
            quiet,
            no_banner,
            no_console,
            no_file,
            no_swagger,
            no_openapi,
            ..
        }) = &cli.command
        {
            Some(Self {
                host: host.clone(),
                port: *port,
                data_dir: data_dir.clone(),
                master_key: master_key.clone(),
                log_level: log_level.clone(),
                env: env.clone(),
                disable_auth: *disable_auth,
                no_color: *no_color,
                quiet: *quiet,
                no_banner: *no_banner,
                no_console: *no_console,
                no_file: *no_file,
                no_swagger: *no_swagger,
                no_openapi: *no_openapi,
            })
        } else {
            None
        }
    }
}

/// Parses the command line.
#[must_use]
pub fn parse_cli() -> Cli {
    Cli::parse()
}

/// Returns the closest match to `input` from `candidates`, if any, using
/// the Jaro-Winkler string similarity. The candidate must score at or
/// above `threshold` (0.0..=1.0) to be returned.
#[must_use]
pub fn did_you_mean<'a>(input: &str, candidates: &'a [&'a str], threshold: f64) -> Option<&'a str> {
    let mut best: Option<(&str, f64)> = None;
    for c in candidates {
        let s = strsim::jaro_winkler(input, c);
        if s >= threshold && best.is_none_or(|(_, bs)| s > bs) {
            best = Some((c, s));
        }
    }
    best.map(|(c, _)| c)
}

/// Known top-level subcommand names. Useful for `--help` and the
/// `did_you_mean` engine.
pub const TOP_LEVEL_SUBCOMMANDS: &[&str] = &["start", "version", "health", "snapshot", "help"];

/// Known snapshot subcommand names.
pub const SNAPSHOT_SUBCOMMANDS: &[&str] = &["create", "list", "info", "delete", "restore", "help"];

/// Returns the subcommand name to suggest for an unknown top-level arg.
#[must_use]
pub fn suggest_top_subcommand(input: &str) -> Option<String> {
    did_you_mean(input, TOP_LEVEL_SUBCOMMANDS, 0.70).map(|s| s.to_string())
}

/// Returns the subcommand name to suggest for an unknown snapshot subcommand.
#[must_use]
pub fn suggest_snapshot_subcommand(input: &str) -> Option<String> {
    did_you_mean(input, SNAPSHOT_SUBCOMMANDS, 0.70).map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_validates() {
        let cfg = AppConfig::default();
        cfg.validate().unwrap();
    }

    #[test]
    fn production_requires_master_key() {
        let mut cfg = AppConfig::default();
        cfg.data.env = "production".into();
        cfg.auth.master_key.clear();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn production_with_master_key_validates() {
        let mut cfg = AppConfig::default();
        cfg.data.env = "production".into();
        cfg.auth.master_key = "abcd".into();
        cfg.validate().unwrap();
    }

    #[test]
    fn cli_overrides_merge() {
        let mut cfg = AppConfig::default();
        let cli = CliOverrides {
            host: Some("127.0.0.1".into()),
            port: Some(9000),
            ..Default::default()
        };
        cfg.merge_cli(&cli);
        assert_eq!(cfg.server.host, "127.0.0.1");
        assert_eq!(cfg.server.port, 9000);
    }

    #[test]
    fn config_loads_default_toml() {
        // Resolve the path relative to the workspace root (cargo runs tests
        // from the package directory).
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest.parent().and_then(|p| p.parent()).unwrap();
        let path = workspace_root.join(DEFAULT_CONFIG_PATH);
        let cfg = AppConfig::load(&path).unwrap();
        assert_eq!(cfg.server.port, 7700);
    }

    #[test]
    fn did_you_mean_picks_close_match() {
        assert_eq!(
            did_you_mean("statr", &["start", "version"], 0.7),
            Some("start")
        );
        assert_eq!(
            did_you_mean("hlth", &["health", "start"], 0.6),
            Some("health")
        );
        assert_eq!(did_you_mean("zzzzz", &["start"], 0.7), None);
    }

    #[test]
    fn suggest_top_subcommand_works() {
        assert_eq!(suggest_top_subcommand("star"), Some("start".to_string()));
        assert_eq!(suggest_top_subcommand("heal"), Some("health".to_string()));
        assert_eq!(suggest_top_subcommand("snap"), Some("snapshot".to_string()));
    }

    #[test]
    fn suggest_snapshot_subcommand_works() {
        assert_eq!(
            suggest_snapshot_subcommand("creat"),
            Some("create".to_string())
        );
        assert_eq!(
            suggest_snapshot_subcommand("delte"),
            Some("delete".to_string())
        );
    }
}
