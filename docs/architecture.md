# AccelerateSearch Architecture

AccelerateSearch is a self-hosted, production-grade search engine written
in Rust. This document describes the high-level architecture, crate
dependency graph, and the data flow during a search request.

## Goals

* **Single binary** that serves the full REST API on Linux, macOS, and
  Windows.
* **Pluggable** storage backend (default: embedded `redb`).
* **Fast** keyword + vector search with a 10ms target latency for
  sub-million-document collections on commodity hardware.
* **Meilisearch-style** developer experience: tasks, settings, scopes,
  webhooks.
* **Elasticsearch-style** power: complex filter expressions, facet stats,
  multi-index search, ranking rules.
* **OpenSearch-level** observability: Prometheus metrics, structured
  logging via `tracing`.

## Crate Dependency Graph

```
                    ┌─────────────────┐
                    │  accelerate     │  (root binary)
                    └────────┬────────┘
                             │
                    ┌────────▼────────┐
                    │    server       │  (HTTP lifecycle, banner)
                    └────────┬────────┘
                             │
        ┌────────────────────┼─────────────────────┐
        │                    │                     │
   ┌────▼─────┐        ┌─────▼──────┐         ┌────▼────┐
   │   api    │        │  scheduler │         │ telemetry│
   └────┬─────┘        └────────────┘         └─────────┘
        │
        ├────────────┬──────────────┬─────────────┐
        │            │              │             │
   ┌────▼─────┐ ┌────▼─────┐  ┌──────▼─────┐  ┌────▼────┐
   │   auth   │ │  search  │  │  indexing  │  │ filters │
   └────┬─────┘ └────┬─────┘  └──────┬─────┘  └────┬────┘
        │            │              │             │
        │       ┌────┴────┐         │             │
        │       │  cache  │         │             │
        │       └─────────┘         │             │
        │                           │             │
   ┌────▼─────┐               ┌──────▼─────┐  ┌────▼────┐
   │ security │               │  storage   │  │ facets │
   └──────────┘               └──────┬─────┘  └─────────┘
                                    │
                              ┌─────▼─────┐
                              │   redb    │
                              └───────────┘
```

## Data Flow: Search Request

1. `actix-web` receives the HTTP request at `/api/v1/collections/{uid}/search`.
2. The middleware stack (rate limit → auth → tracing) runs.
3. The `search` handler validates the request and checks that the
   collection exists in the `CollectionStore`.
4. `SearchEngine::search_with_rules` checks the result cache. On hit,
   the cached response is returned immediately.
5. On miss, the engine:
   * Loads the collection's `InvertedIndex` from the `IndexStore` (cached
     in a `DashMap` keyed by `CollectionId`).
   * Resolves synonym expansion for the query terms.
   * Applies typo tolerance (bounded expansion).
   * Scores candidates with BM25.
   * Runs the filter (recursive-descent parser → evaluator) on hydrated
     documents.
   * Applies user-requested sorting and ruleset (pinned, hidden, overrides).
   * Computes facet distributions.
6. The response is JSON-serialised and returned with `processingTimeMs`.
7. Successful responses are stored in the result cache (TTL + LRU).

## Data Flow: Indexing Request

1. `POST /api/v1/collections/{uid}/documents` is received.
2. The `documents` handler validates each document and calls
   `DocumentService::add_or_replace`.
3. The service runs the `IndexingPipeline` which:
   * Tokenises each searchable field with the `Analyzer` (Unicode NFC,
     lowercase, stop-word removal, stemming).
   * Updates the in-memory `InvertedIndex` (per-field term frequencies,
     per-document field lengths).
   * Recomputes BM25 collection statistics.
   * Rebuilds the FST-backed term dictionary.
   * Persists the documents to `storage::TABLE_DOCUMENTS` and the
     inverted-index snapshot to `storage::TABLE_INDEX`.
4. The result cache is invalidated for the collection.

## Concurrency Model

* The server runs `actix-web` with one Tokio worker per CPU core.
* Shared in-memory state lives in `DashMap` instances (per-collection
  indexes, hooks, rulesets, key cache).
* Long-running mutators (e.g. `RwLock` over an index) use
  `parking_lot` for lower contention than `std::sync`.
* Background jobs (`scheduler`) run on a `Notify`-gated Tokio task that
  can be cancelled on shutdown.
* Result caching uses an LRU + TTL `TtlCache` (`parking_lot::Mutex`).
* Hot-reloadable config is wrapped in `arc-swap` so readers never block
  writers.

## Storage

The default `StorageBackend` is an embedded `redb` key-value store. The
schema is table-based, with the following tables:

| Table | Key | Value |
| --- | --- | --- |
| `collections` | `CollectionId` | `Collection` (JSON) |
| `documents`   | `{collection}\u{0}{doc_id}` | raw document bytes |
| `inverted_index` | `CollectionId` | `IndexRecord` (JSON) |
| `tasks`       | `TaskId` | `Task` (JSON) |
| `keys`        | `ApiKeyId` | `ApiKey` (JSON) |
| `snapshots`   | `SnapshotName` | `SnapshotMeta` (JSON) |
| `synonyms`    | `{collection}\u{0}{term}` | synonym entries |
| `hooks`       | `Uuid` | `Hook` (JSON) |
| `search_rules`| `CollectionId` | `Ruleset` (JSON) |

A different backend (e.g. RocksDB, Sled) can be plugged in by
implementing the `StorageBackend` trait and replacing the wiring in
`server::run`.

## Configuration

`crates/config` parses `config/default.toml` (or the path supplied via
`--config`), then layers CLI overrides on top. Validation is performed
with the `validator` crate. See `docs/configuration.md` for the full key
reference.

## Security

* Master key (SHA-256 hashed) gates all `/api/v1/*` routes except a
  whitelist (`/health`, `/version`, `/metrics`, `/swagger-ui/*`).
* API keys are scoped by `Permission` and optional collection list.
* Rate limiting uses `governor` keyed by client IP.
* Security headers (`X-Content-Type-Options`, `CSP`, HSTS, …) are added
  on every response.
* All user-supplied strings are sanitised (control characters stripped)
  before storage.
* Tenant tokens are HS256 JWTs with a 1-hour maximum lifetime.
