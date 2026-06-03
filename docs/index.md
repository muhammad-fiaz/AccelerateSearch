# AccelerateSearch

A self-hosted, production-grade search engine written in Rust.

AccelerateSearch combines the developer experience of Meilisearch with
the analytical power of Elasticsearch, all in a single binary that
runs on Linux, macOS, and Windows.

## Features

- **Blazing-fast full-text search** with BM25 ranking
- **Lock-free concurrent reads** with `DashMap` and `parking_lot`
- **FST-backed term dictionaries** for O(log n) prefix lookups and
  autocomplete
- **Vector and hybrid search** with scalar / product / binary
  quantization
- **Fuzzy matching** with bounded Damerau-Levenshtein typo tolerance
- **Complex filter expressions**
  (`field = "value" AND rating > 4 OR location GEO_BBOX …`)
- **Facet distributions and stats** for every field
- **Per-collection settings**: ranking rules, synonyms, stop words,
  typo tolerance, embedders, distinct field, …
- **Webhooks** that fire on document and index events
- **Tenant tokens** for short-lived, scoped access from the browser
- **API keys** with expiry, scopes, and per-collection ACLs
- **Prometheus metrics** at `/metrics`, structured logs via `tracing`
- **Snapshots** (tar + zstd) for backup and restore
- **Single binary** with no external services required

## Where to go next

- [Architecture overview](./architecture.md)
- [REST API reference](./api.md)
- [Configuration reference](./configuration.md)
- [Deployment guide](./deployment.md)
- [Development guide](./development.md)
- [Rust API docs (cargo doc)](./rust-api.md)

## Project layout

```
crates/
  api/          REST handlers, DTOs, OpenAPI schema
  auth/         master key, API keys, tenant tokens
  cache/        LRU + TTL cache
  config/       TOML config & validation
  documents/    document service
  filters/      filter expression parser & evaluator
  hybrid/       RRF, score normalization
  highlighting/ <em> highlighting
  indexing/     tokenization + inverted index + FST
  metrics/      Prometheus exporter
  models/       shared data types
  search/       BM25, ranking, query parser
  security/     rate limit, CORS, audit logger
  server/       HTTP lifecycle, banner
  storage/      StorageBackend trait + redb
  tasks/        async task queue
  telemetry/    tracing-subscriber setup
  typo/         Damerau-Levenshtein
  utils/        hash, random, time helpers
  validation/   input validation & sanitization
  vector/       embedding types + quantization
config/         default.toml
docs/           this mdbook source
.github/        CI + release workflows
```

## Author

Muhammad Fiaz — <contact@muhammadfiaz.com>

## License

[Apache-2.0](https://www.apache.org/licenses/LICENSE-2.0)
