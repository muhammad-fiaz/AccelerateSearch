# Configuration Reference

Configuration is parsed at startup from (in order of precedence):

1. CLI flags (e.g. `--host 0.0.0.0 --port 7700`)
2. Environment variables (e.g. `ACCELERATE_HOST`, `ACCELERATE_PORT`)
3. `config/default.toml` (the file shipped with the binary)
4. Built-in defaults

The path to the TOML file can be overridden with `--config <file>` or
`ACCELERATE_CONFIG=/path/to/file`.

> [!WARNING]
> The project is in active development. Configuration keys may be
> renamed, removed, or have their defaults changed between releases.
> Always read the [`config/default.toml`](https://github.com/muhammad-fiaz/AccelerateSearch/blob/main/config/default.toml)
> of the release you are running for the authoritative reference.

## `[server]`

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `host` | `string` | `localhost` | Bind address. `localhost`/`127.0.0.1` for loopback, `0.0.0.0` to listen on every interface. |
| `port` | `u16` | `7700` | TCP port (Meilisearch-compatible). |
| `workers` | `usize` | `0` | Actix worker threads. `0` = auto = number of CPU cores. |
| `max_connections` | `usize` | `0` | Maximum simultaneous connections. `0` = unlimited. |
| `keep_alive` | `string` | `75s` | HTTP keep-alive duration. |
| `read_timeout` | `string` | `30s` | Maximum time to wait for a request. |
| `write_timeout` | `string` | `30s` | Maximum time to wait for a response. |
| `shutdown_timeout` | `string` | `10s` | Graceful shutdown window. |
| `max_body_size` | `usize` | `104857600` | Max HTTP request body in bytes (default 100 MiB). |

### `[server.tls]`

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `enabled` | `bool` | `false` | Enable TLS on the listen socket. |
| `cert_path` | `string` | `""` | PEM certificate chain path. |
| `key_path` | `string` | `""` | PEM private key path. |
| `ca_cert_path` | `string` | `""` | Optional mTLS CA bundle. |
| `require_client_cert` | `bool` | `false` | Enforce mTLS client certificates. |

## `[api_docs]`

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `swagger_ui_enabled` | `bool` | `true` | Serve Swagger UI at `/swagger-ui/`. |
| `openapi_enabled` | `bool` | `true` | Serve the OpenAPI spec at `/api-docs/openapi.json`. |

## `[data]`

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `dir` | `string` | `./data` | On-disk `redb` database directory. |
| `env` | `string` | `development` | `development` or `production`. Production requires a non-empty `auth.master_key`. |

## `[auth]`

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `master_key` | `string` | `""` | Master API key for admin access. Set with `ACCELERATE_MASTER_KEY` in production. |
| `disable_auth` | `bool` | `false` | Explicitly disable authentication (development only). |

## `[search]`

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `max_values_per_facet` | `usize` | `100` | Max facet values returned per field. |
| `pagination_max_total_hits` | `usize` | `1000` | Max total hits reported in a paginated response. |
| `bm25_k1` | `f32` | `1.2` | BM25 term-frequency saturation. |
| `bm25_b` | `f32` | `0.75` | BM25 length normalisation. |
| `default_limit` | `usize` | `20` | Default page size when not supplied by the client. |
| `max_limit` | `usize` | `1000` | Maximum page size accepted from the client. |

## `[indexing]`

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `max_batch_size` | `usize` | `1000` | Max documents per indexing batch. |
| `commit_interval_ms` | `u64` | `500` | Force commit after this delay. |
| `parallelism` | `usize` | `0` | Indexing pipeline parallelism. `0` = auto. |
| `stem` | `bool` | `true` | Apply language-aware stemming. |
| `remove_stop_words` | `bool` | `true` | Strip stop words before indexing. |

## `[vector]`

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `enabled` | `bool` | `false` | Enable vector search at the platform level. |
| `dimensions` | `usize` | `384` | Default embedding dimensions. |
| `similarity` | `string` | `cosine` | `cosine`, `dot`, or `euclidean`. |
| `hnsw_m` | `usize` | `16` | HNSW connections per node. |
| `hnsw_ef_construction` | `usize` | `200` | HNSW search depth during indexing. |
| `hnsw_ef_search` | `usize` | `50` | HNSW search depth during queries. |
| `quantization` | `string` | `none` | `none`, `scalar`, `product`, or `binary`. |
| `pq_m` | `usize` | `8` | Sub-spaces for product quantization. |
| `pq_k` | `usize` | `256` | Centroids per sub-space for product quantization. |
| `allow_sparse` | `bool` | `true` | Allow sparse vector embeddings (SPLADE-style). |
| `allow_multi` | `bool` | `true` | Allow multi-vector embeddings (ColBERT-style). |
| `embedder_url` | `string` | `""` | Optional external embedder URL for auto-embedding. |
| `embedder_model` | `string` | `""` | Optional embedder model name for telemetry. |

## `[logging]`

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `level` | `string` | `info` | `trace`, `debug`, `info`, `warn`, `error`. |
| `format` | `string` | `pretty` | `pretty` or `json`. |
| `dir` | `string` | `./logs` | Log file directory. |
| `file_prefix` | `string` | `accelerate` | Log file prefix (`{prefix}.{date}.log`). |
| `max_files` | `usize` | `7` | Retained log files. `0` = unlimited. |
| `max_size_mb` | `usize` | `100` | Max single-file size in MB. `0` = unlimited. |
| `auto_delete_days` | `usize` | `30` | Auto-delete logs older than N days. `0` = never. |
| `no_console` | `bool` | `false` | Disable console output. |
| `no_file` | `bool` | `false` | Disable the file log appender. |
| `no_color` | `bool` | `false` | Strip ANSI color from console output. |
| `quiet` | `bool` | `false` | Silence non-error log lines. |

## `[metrics]`

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `enabled` | `bool` | `true` | Expose Prometheus metrics at `/metrics`. |
| `endpoint` | `string` | `/metrics` | Metrics endpoint path. |

## `[snapshots]`

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `dir` | `string` | `./snapshots` | Snapshot directory. |
| `schedule` | `string` | `0 0 * * *` | Cron schedule for auto-snapshot (UTC). |
| `auto_create` | `bool` | `false` | Enable automatic snapshot creation. |

## `[updates]`

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `check_enabled` | `bool` | `true` | Check for new versions on startup. |
| `check_interval` | `string` | `24h` | Interval between version checks. |

## `[rate_limit]`

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `enabled` | `bool` | `true` | Enable per-client rate limiting. |
| `requests_per_second` | `u32` | `100` | Steady-state RPS per client. |
| `burst_size` | `u32` | `200` | Allowed burst above the steady-state RPS. |

## `[telemetry]`

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `tracing_enabled` | `bool` | `true` | Enable distributed tracing. |
| `service_name` | `string` | `accelerate` | Service name reported to tracing backends. |

## `[cache]`

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `enabled` | `bool` | `true` | Toggle the search result cache. |
| `max_entries` | `usize` | `10000` | Maximum number of cached entries. |
| `ttl_seconds` | `u64` | `300` | Cache TTL in seconds. |

## `[cors]`

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `enabled` | `bool` | `true` | Apply CORS headers on every response. |
| `allowed_origins` | `array<string>` | `[]` | Allowed origins. `[]` = allow all. |
| `allowed_methods` | `array<string>` | `["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"]` | Allowed methods. |
| `allowed_headers` | `array<string>` | `["Authorization", "Content-Type", "Accept", "Origin", "X-Requested-With"]` | Allowed request headers. |
| `allow_credentials` | `bool` | `true` | Allow cookies / authorization headers. |
| `max_age` | `u64` | `3600` | Preflight cache duration in seconds. |

## Environment variables

Every CLI flag is also exposed as an `ACCELERATE_*` environment
variable. For example, `--host 0.0.0.0` is equivalent to
`ACCELERATE_HOST=0.0.0.0`. Sensitive values that are commonly set via
the environment in production:

| Variable | Equivalent CLI flag |
| --- | --- |
| `ACCELERATE_CONFIG` | `--config <file>` |
| `ACCELERATE_MASTER_KEY` | `--master-key <key>` |
| `ACCELERATE_HOST` | `--host <addr>` |
| `ACCELERATE_PORT` | `--port <n>` |
| `ACCELERATE_DATA_DIR` | `--data-dir <dir>` |
| `ACCELERATE_LOG_LEVEL` | `--log-level <level>` |
| `ACCELERATE_ENV` | `--env <development\|production>` |

## Precedence example

```bash
# config/default.toml contains:  host = "localhost", port = 7700
# env vars:                       ACCELERATE_PORT=8080
# CLI:                            --host 0.0.0.0

# Effective:
#   host = 0.0.0.0   (CLI > env > TOML > default)
#   port = 8080      (env > TOML > default)
```
