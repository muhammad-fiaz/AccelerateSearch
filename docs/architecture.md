# AccelerateSearch Architecture

AccelerateSearch is a self-hosted, production-grade search engine written
in Rust. This document describes the high-level architecture, the crate
dependency graph, and the data flow during a search and an indexing
request.

## Goals

* **Single binary** that serves the full REST API on Linux, macOS, and
  Windows.
* **Pluggable** storage backend (default: embedded `redb`).
* **Fast** keyword + vector search with a 10 ms target p99 for
  sub-million-document collections on commodity hardware.
* **Meilisearch-style** developer experience: tasks, settings, scopes,
  webhooks, tenant tokens.
* **Elasticsearch-style** power: complex filter expressions, facet
  distributions, multi-index search, ranking rules.
* **OpenSearch-level** observability: Prometheus metrics, structured
  logging via `tracing`.

## Crate Dependency Graph

The workspace contains 30 library crates and one binary. The binary
(`accelerate`) is a thin shell that wires them together; the server
lifecycle (`crates/server`) owns the actix-web setup, banner, and
graceful-shutdown logic. Everything else is layered on top of the
`api` crate, which holds the HTTP handlers.

```
                      ┌─────────────────────┐
                      │      accelerate     │  (root binary)
                      └──────────┬──────────┘
                                 │
                      ┌──────────▼──────────┐
                      │       server        │  (HTTP lifecycle, banner)
                      └──────────┬──────────┘
                                 │
            ┌────────────────────┼────────────────────┐
            │                    │                    │
     ┌──────▼──────┐     ┌───────▼───────┐    ┌───────▼───────┐
     │     api     │     │  scheduler    │    │  telemetry    │
     └──────┬──────┘     └───────────────┘    └───────────────┘
            │
   ┌────────┼────────┬───────────┬────────────┬──────────────┐
   │        │        │           │            │              │
┌──▼──┐  ┌──▼──┐  ┌───▼───┐  ┌────▼────┐  ┌────▼────┐  ┌─────▼─────┐
│auth │  │search│  │indexing│  │documents│  │filters │  │ collections│
└──┬──┘  └──┬──┘  └───┬───┘  └────┬────┘  └────┬────┘  └─────┬─────┘
   │        │         │           │            │             │
   │    ┌───▼───┐     │           │            │             │
   │    │ cache │     │           │            │             │
   │    └───────┘     │           │            │             │
   │                 │           │            │             │
┌──▼──────┐     ┌────▼────┐  ┌────▼────┐  ┌────▼────┐  ┌────▼────┐
│security │     │ storage │  │ facets  │  │  typo   │  │  hybrid │
└─────────┘     └────┬────┘  └─────────┘  └─────────┘  └─────────┘
                     │
              ┌──────▼──────┐
              │    redb     │  (embedded key-value store)
              └─────────────┘
```

Cross-cutting helpers that all crates can depend on:

| Crate | Role |
| --- | --- |
| `errors`     | Unified `AppError` / `AppResult` with `From` impls |
| `utils`      | Hash, random, time helpers |
| `models`     | Shared DTOs and value types |
| `validation` | Collection-uid, field-name, query, and filter validation + sanitisation |
| `highlighting` | `<em>`-style snippet builder |
| `synonyms`   | Synonym map storage and lookup |
| `vector`     | `Embedding` enum + scalar / product / binary quantisation |
| `metrics`    | Prometheus exporter |
| `cache`      | LRU + TTL cache for search results |
| `tasks`      | Async task queue with cancellation |
| `snapshots`  | tar + zstd snapshot read / write |
| `telemetry`  | `tracing-subscriber` setup with daily file rotation |
| `cluster`, `replication`, `sharding` | Skeleton traits with `// TODO(<scope>)` markers |

## Data Flow: Search Request

1. `actix-web` receives the HTTP request at
   `/api/v1/collections/{uid}/search`.
2. The middleware stack runs in order: tracing → rate limit → auth.
3. The `search` handler validates the request and looks up the
   collection in the in-memory `CollectionStore`.
4. `SearchEngine::search_with_rules` consults the result cache. On hit,
   the cached response is returned immediately.
5. On miss, the engine:
   * Loads the collection's `InvertedIndex` from the `IndexStore`
     (cached in a `DashMap` keyed by `CollectionId`).
   * Resolves synonym expansion for the query terms.
   * Applies typo tolerance (bounded Damerau-Levenshtein expansion).
   * Scores candidates with BM25 (`crates/search::bm25`).
   * Applies the filter (recursive-descent parser → evaluator) on
     hydrated documents.
   * Applies user-requested sorting and the ruleset
     (pinned, hidden, sort/filter overrides).
   * Computes facet distributions.
6. The response is JSON-serialised with a `processingTimeMs` field
   and returned.
7. Successful responses are stored in the result cache (TTL + LRU).

## Data Flow: Indexing Request

1. `POST /api/v1/collections/{uid}/documents` is received.
2. The `documents` handler validates every document and calls
   `DocumentService::add_or_replace`.
3. The service runs the `IndexingPipeline` which:
   * Tokenises each searchable field with the `Analyzer` (Unicode NFC,
     lowercase, stop-word removal, optional stemming).
   * Updates the in-memory `InvertedIndex` (per-field term frequencies,
     per-document field lengths).
   * Recomputes BM25 collection statistics.
   * Rebuilds the FST-backed term dictionary for O(log n) prefix
     lookups (used by autocomplete).
   * Persists the documents to `storage::TABLE_DOCUMENTS` and the
     postings / terms / field-lengths / stats to the matching
     `TABLE_*` tables.
4. The result cache is invalidated for the collection.

## Concurrency Model

* The server runs `actix-web` with one Tokio worker per CPU core.
* Shared in-memory state lives in `DashMap` instances (per-collection
  indexes, hooks, rulesets, key cache).
* Long-running mutators (e.g. `RwLock` over an index) use
  `parking_lot` for lower contention than `std::sync`.
* Background jobs (`scheduler`) run on a `Notify`-gated Tokio task
  that can be cancelled on shutdown.
* Result caching uses an LRU + TTL `TtlCache`
  (`parking_lot::Mutex<LruCache<K, Entry<V>>>`).
* Hot-reloadable config is wrapped in `arc-swap` so readers never
  block writers.

## Storage

The default `StorageBackend` is an embedded `redb` key-value store.
The schema is table-based, with the following tables defined in
`crates/storage`:

| Table | Key | Value |
| --- | --- | --- |
| `collections`     | `CollectionId`                     | `Collection` (JSON) |
| `documents`       | `{collection}\u{0}{doc_id}`        | raw document bytes |
| `inverted_index`  | `CollectionId`                     | `IndexRecord` (JSON snapshot) |
| `postings`        | `{collection}\u{0}{term}`         | per-doc posting list |
| `terms`           | `{collection}\u{0}{term}`         | term metadata (df, total tf) |
| `field_lengths`   | `{collection}\u{0}{doc_id}`       | per-doc field lengths |
| `collection_stats`| `CollectionId`                     | `CollectionStats` |
| `vectors`         | `{collection}\u{0}{doc_id}`       | raw vector bytes |
| `tasks`           | `TaskId`                           | `Task` (JSON) |
| `keys`            | `ApiKeyId`                         | `ApiKey` (JSON) |
| `settings`        | `CollectionId`                     | `CollectionSettings` (JSON) |
| `snapshots`       | `SnapshotName`                     | `SnapshotMeta` (JSON) |
| `synonyms`        | `{collection}\u{0}{term}`         | synonym entries |

A different backend (RocksDB, Sled, …) can be plugged in by
implementing the `StorageBackend` trait and swapping the wiring in
`crates/server::run`.

## Configuration

`crates/config` parses `config/default.toml` (or the path supplied via
`--config`), then layers CLI overrides on top, then environment
variables, then the built-in defaults. Validation is performed with
the `validator` crate. See `docs/configuration.md` for the full key
reference.

## Security

* Master key (SHA-256 hashed) gates all `/api/v1/*` routes except a
  whitelist (`/health`, `/version`, `/metrics`, `/swagger-ui/*`).
* API keys are scoped by `Permission` and an optional collection list.
* Tenant tokens are HS256 JWTs with a short (≤ 1 h) lifetime.
* Rate limiting uses `governor` keyed by client IP.
* Security headers (`X-Content-Type-Options`, `CSP`, HSTS, …) are added
  on every response.
* All user-supplied strings are sanitised (control characters stripped,
  whitespace runs collapsed) before storage or query parsing.
