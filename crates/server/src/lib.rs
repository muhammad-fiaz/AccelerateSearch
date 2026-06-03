//! Server lifecycle: startup, banner, graceful shutdown.

mod banner;

use std::sync::Arc;
use std::time::Duration;

use actix_web::{App, HttpServer, web};
use anyhow::Context;
use tokio::sync::Notify;
use tracing::info;
use tracing_actix_web::TracingLogger;
use utoipa::OpenApi;

use api::AppState;
use auth::AuthService;
use collections::CollectionStore;
use config_crate::{AppConfig, CliCommand, CliOverrides};
use documents::DocumentService;
use errors::{AppError, AppResult};
use indexing::{IndexStore, IndexingPipeline};
use search::SearchEngine;
use snapshots::SnapshotService;
use storage::RedbStorage;
use tasks::{Task, TaskHandler, TaskQueue};
use telemetry::{self, TelemetryGuard};
use utils::ensure_dir;
use vector::VectorIndexStore;

/// The main entry point. Wires together every crate, starts the HTTP server,
/// and waits for a shutdown signal.
pub async fn run() -> AppResult<()> {
    let cli = config_crate::parse_cli();
    match &cli.command {
        Some(CliCommand::Version) => {
            println!("accelerate {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Some(CliCommand::Health { url }) => return run_health(url).await,
        Some(CliCommand::Snapshot { action }) => {
            return run_snapshot_action(action).await;
        }
        Some(CliCommand::Start { .. }) | None => {}
    }

    // Load and merge configuration.
    let config_path = cli
        .command
        .as_ref()
        .and_then(|c| match c {
            CliCommand::Start { config, .. } => config.clone(),
            _ => None,
        })
        .unwrap_or_else(|| std::path::PathBuf::from(config_crate::DEFAULT_CONFIG_PATH));

    let mut config = AppConfig::load(&config_path)
        .with_context(|| format!("loading config from {}", config_path.display()))
        .map_err(|e| AppError::ConfigError(e.to_string()))?;
    if let Some(overrides) = CliOverrides::from_cli(&cli) {
        config.merge_cli(&overrides);
    }
    config
        .validate()
        .map_err(|e| AppError::ConfigError(e.to_string()))?;

    // Telemetry.
    let _telemetry_guard: TelemetryGuard = telemetry::init(&config.logging)?;

    // Banner (skipped with --no-banner or --quiet).
    if !should_hide_banner(&cli) {
        print_banner(&config);
    }

    // Storage.
    ensure_dir(&config.data.dir).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_path = config.data.dir.join("accelerate.redb");
    let backend = Arc::new(RedbStorage::open(&db_path)?);
    let storage: Arc<dyn storage::StorageBackend> = backend.clone();

    // Metrics.
    accelerate_metrics::init();

    // Services.
    let auth = Arc::new(AuthService::new(storage.clone(), &config.auth.master_key));
    let collections = Arc::new(CollectionStore::new(storage.clone()));
    collections.load_all().await?;
    let index_store = Arc::new(IndexStore::new(storage.clone()));
    let pipeline = Arc::new(IndexingPipeline::new(index_store.clone(), storage.clone()));
    let documents = Arc::new(DocumentService::new(collections.clone(), pipeline.clone()));
    let search = Arc::new(SearchEngine::new(
        index_store.clone(),
        config.search.clone(),
    ));
    let tasks = Arc::new(TaskQueue::new(storage.clone()));
    let snapshots = Arc::new(SnapshotService::new(
        storage.clone(),
        config.snapshots.clone(),
    ));
    let vectors = Arc::new(VectorIndexStore::new());

    // Background workers.
    spawn_background_workers(config.clone(), tasks.clone(), auth.clone());

    // Update checker.
    if config.updates.check_enabled {
        spawn_update_checker();
    }

    // App state.
    let state = AppState {
        storage: storage.clone(),
        auth: auth.clone(),
        collections: collections.clone(),
        documents: documents.clone(),
        indexes: index_store.clone(),
        search: search.clone(),
        tasks: tasks.clone(),
        snapshots: snapshots.clone(),
        vectors: vectors.clone(),
        config: Arc::new(config.clone()),
    };

    // HTTP server.
    let keep_alive = config
        .server
        .keep_alive
        .parse::<u64>()
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(75));
    let workers = if config.server.workers == 0 {
        num_cpus_or_one()
    } else {
        config.server.workers
    };
    let max_body = config.server.max_body_size;
    let server = HttpServer::new(move || build_app(state.clone(), max_body))
        .workers(workers)
        .keep_alive(keep_alive)
        .max_connections(config.server.max_connections)
        .client_request_timeout(
            config
                .server
                .read_timeout
                .parse::<u64>()
                .map(Duration::from_secs)
                .unwrap_or(Duration::from_secs(30)),
        )
        .bind((config.server.host.clone(), config.server.port))
        .map_err(|e| AppError::Internal(format!("bind: {e}")))?
        .run();

    info!(
        host = %config.server.host,
        port = config.server.port,
        workers,
        "accelerate is running"
    );

    // Wait for shutdown.
    let stop = Arc::new(Notify::new());
    let stop_for_signal = stop.clone();
    tokio::spawn(async move {
        wait_for_signal().await;
        info!("shutdown signal received");
        stop_for_signal.notify_waiters();
    });
    let server_handle = server.handle();
    let stop_for_server = stop.clone();
    tokio::spawn(async move {
        stop_for_server.notified().await;
        server_handle.stop(true).await;
    });
    server
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(())
}

fn build_app(
    state: AppState,
    max_body: usize,
) -> App<
    impl actix_web::dev::ServiceFactory<
        actix_web::dev::ServiceRequest,
        Config = (),
        Response = actix_web::dev::ServiceResponse<impl actix_web::body::MessageBody>,
        Error = actix_web::Error,
        InitError = (),
    >,
> {
    let mut app = App::new()
        .app_data(web::JsonConfig::default().limit(max_body))
        .app_data(web::PayloadConfig::new(max_body))
        .app_data(web::Data::new(state.clone()))
        .wrap(TracingLogger::default())
        .wrap(security::CorsMiddleware::new(state.config.cors.clone()))
        .wrap(security::SecurityHeadersMiddleware)
        .wrap(security::RateLimitMiddleware::new(Arc::new(
            security::RateLimiterPool::new(&state.config.rate_limit),
        )))
        .wrap(auth::AuthMiddleware::new(state.auth.clone()))
        .configure(api::configure_routes)
        .configure(api::configure_root);

    // Conditionally add Swagger UI and OpenAPI spec
    if state.config.api_docs.openapi_enabled && state.config.api_docs.swagger_ui_enabled {
        app = app.service(
            utoipa_swagger_ui::SwaggerUi::new("/swagger-ui/{_:.*}")
                .url("/api-docs/openapi.json", api::openapi::ApiDoc::openapi()),
        );
    }

    app
}

fn num_cpus_or_one() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

fn print_banner(config: &AppConfig) {
    if config.logging.quiet {
        return;
    }
    let color = utils::color::stdout_is_colored() && !config.logging.no_color;
    if !config.logging.no_console {
        let banner = crate::banner::render(
            color,
            env!("CARGO_PKG_VERSION"),
            &config.server.host,
            config.server.port,
            &config.data.env,
            &config.data.dir.display().to_string(),
        );
        println!("{}", banner);
    }
}

async fn wait_for_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut term = signal(SignalKind::terminate()).expect("install SIGTERM handler");
        let mut int = signal(SignalKind::interrupt()).expect("install SIGINT handler");
        tokio::select! {
            _ = term.recv() => {}
            _ = int.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

async fn run_health(url: &str) -> AppResult<()> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{url}/health"))
        .send()
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    if resp.status().is_success() {
        println!("OK");
        Ok(())
    } else {
        Err(AppError::Internal(format!(
            "health check failed: {}",
            resp.status()
        )))
    }
}

async fn run_snapshot_action(action: &config_crate::SnapshotAction) -> AppResult<()> {
    match action {
        config_crate::SnapshotAction::Create { url, master_key } => {
            let client = reqwest::Client::new();
            let mut req = client.post(format!("{url}/api/v1/snapshots"));
            if let Some(k) = master_key {
                req = req.bearer_auth(k);
            }
            let resp = req
                .send()
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            println!("{status}\n{body}");
            Ok(())
        }
        config_crate::SnapshotAction::List { url, master_key } => {
            let client = reqwest::Client::new();
            let mut req = client.get(format!("{url}/api/v1/snapshots"));
            if let Some(k) = master_key {
                req = req.bearer_auth(k);
            }
            let resp = req
                .send()
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            println!("{status}\n{body}");
            Ok(())
        }
        config_crate::SnapshotAction::Info {
            name,
            url,
            master_key,
        } => {
            let client = reqwest::Client::new();
            let mut req = client.get(format!("{url}/api/v1/snapshots/{name}"));
            if let Some(k) = master_key {
                req = req.bearer_auth(k);
            }
            let resp = req
                .send()
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            println!("{status}\n{body}");
            Ok(())
        }
        config_crate::SnapshotAction::Delete {
            name,
            url,
            master_key,
        } => {
            let client = reqwest::Client::new();
            let mut req = client.delete(format!("{url}/api/v1/snapshots/{name}"));
            if let Some(k) = master_key {
                req = req.bearer_auth(k);
            }
            let resp = req
                .send()
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            println!("{status}\n{body}");
            Ok(())
        }
        config_crate::SnapshotAction::Restore { path, data_dir } => {
            let db_path = data_dir.join("accelerate.redb");
            let backend = Arc::new(RedbStorage::open(&db_path)?);
            let cfg = config_crate::SnapshotsConfig::default();
            let svc = SnapshotService::new(backend, cfg);
            svc.restore(path, &db_path).await
        }
    }
}

fn spawn_background_workers(config: AppConfig, tasks: Arc<TaskQueue>, _auth: Arc<AuthService>) {
    let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
    tokio::spawn(async move {
        tasks
            .clone()
            .run_worker(stop_rx, Arc::new(NoopTaskHandler))
            .await;
    });
    let stop = Arc::new(Notify::new());
    let stop_for_log = stop.clone();
    let log_dir = config.logging.dir.clone();
    let days = config.logging.auto_delete_days;
    tokio::spawn(async move {
        scheduler::run_interval(
            Arc::new(scheduler::LogCleanupJob { log_dir, days }),
            Duration::from_secs(86_400),
            stop_for_log,
        )
        .await;
    });
    let _ = stop_tx; // keep alive for lifetime of program
}

struct NoopTaskHandler;

#[async_trait::async_trait]
impl TaskHandler for NoopTaskHandler {
    async fn run(&self, _task: &Task) -> AppResult<u64> {
        Ok(0)
    }
}

fn spawn_update_checker() {
    tokio::spawn(async move {
        let url = "https://api.github.com/repos/muhammad-fiaz/acceleratesearch/releases/latest";
        let client = match reqwest::Client::builder()
            .user_agent("accelerate-update-checker")
            .timeout(Duration::from_secs(5))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                tracing::debug!("update checker client build failed: {e}");
                return;
            }
        };
        let resp = match client.get(url).send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::debug!("update checker request failed: {e}");
                return;
            }
        };
        if !resp.status().is_success() {
            tracing::debug!("update checker got non-success: {}", resp.status());
            return;
        }
        let json = match resp.json::<serde_json::Value>().await {
            Ok(j) => j,
            Err(e) => {
                tracing::debug!("update checker parse failed: {e}");
                return;
            }
        };
        let Some(tag) = json.get("tag_name").and_then(|v| v.as_str()) else {
            return;
        };
        let current = env!("CARGO_PKG_VERSION");
        let remote = tag.trim_start_matches('v');
        if remote == current {
            return;
        }
        let release_url = json
            .get("html_url")
            .and_then(|v| v.as_str())
            .unwrap_or("https://github.com/muhammad-fiaz/acceleratesearch/releases");
        info!(
            "A new version of AccelerateSearch is available: v{remote} (current: v{current}). {release_url}"
        );
    });
}

fn should_hide_banner(cli: &config_crate::Cli) -> bool {
    if let Some(CliCommand::Start {
        no_banner, quiet, ..
    }) = &cli.command
    {
        return *no_banner || *quiet;
    }
    false
}
