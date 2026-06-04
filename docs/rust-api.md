# Rust API Reference

The full Rust API documentation is generated with `cargo doc` and
lives in the `target/doc/` directory of the workspace. It is
regenerated on every push to `main` by the GitHub Actions docs
workflow and published at:

> <https://muhammad-fiaz.github.io/AccelerateSearch/rust-api/>

## Build locally

```bash
cargo doc --no-deps --workspace --target-dir target
# Optional: open in your default browser
cargo doc --no-deps --workspace --target-dir target --open
```

`RUSTDOCFLAGS="-D warnings"` is set in the CI, so any doc warning
fails the build.

## Crate index

| Crate | Description |
| --- | --- |
| `accelerate` | Root binary |
| `api` | HTTP handlers and DTOs |
| `auth` | Master key, API keys, tenant tokens |
| `cache` | LRU + TTL cache |
| `cluster` | Cluster skeleton |
| `collections` | Collection metadata service |
| `config` | TOML configuration |
| `documents` | Document service |
| `errors` | Unified error type |
| `facets` | Facet distribution engine |
| `filters` | Filter expression parser & evaluator |
| `hybrid` | Hybrid query fusion (RRF) |
| `highlighting` | `<em>` highlighting |
| `indexing` | Tokenisation, inverted index, FST |
| `metrics` | Prometheus exporter |
| `models` | Shared data types |
| `replication` | Replication skeleton |
| `scheduler` | Cron + interval jobs |
| `search` | BM25, ranking, query parser |
| `security` | Rate limit, CORS, audit logger |
| `server` | HTTP lifecycle, banner |
| `sharding` | Sharding skeleton |
| `snapshots` | Tar + zstd snapshots |
| `storage` | `StorageBackend` trait + redb |
| `synonyms` | Synonym expansion |
| `tasks` | Async task queue |
| `telemetry` | tracing-subscriber setup |
| `typo` | Damerau-Levenshtein |
| `utils` | Hash, random, time helpers |
| `validation` | Input validation & sanitization |
| `vector` | Embedding types + quantization |
