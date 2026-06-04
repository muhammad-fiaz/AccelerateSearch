# Development

## Prerequisites

* Rust 1.85+ (edition 2024; 1.88+ recommended for the latest dependencies)
* A C toolchain (gcc, clang, or MSVC)
* On Linux: `pkg-config` and `mimalloc`'s usual build deps
* `git`, `curl`, and `cargo` (the toolchain)

## Build

```bash
git clone https://github.com/muhammad-fiaz/AccelerateSearch
cd AccelerateSearch
cargo build
```

The binary lands at `target/debug/accelerate`.

## Run

```bash
cargo run -- --config config/default.toml
```

Logs are emitted to stdout and `logs/accelerate-YYYY-MM-DD.log`.

## Test

```bash
cargo test --workspace --no-fail-fast
```

* Unit tests live next to the code they cover (`#[cfg(test)] mod tests`).
* Property-based tests use `proptest` for the filter parser
  (`filters::evaluator`) and the tokenizer
  (`indexing::analyzer`).
* Every crate that exposes a public type is documented with
  `cargo doc --no-deps --workspace`; the CI fails on any
  rustdoc warning.

## Lint and format

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

The CI workflow runs both on every push; failing either blocks the
build.

## Benchmarks

`benchmark/` is a standalone project that spins up a 1M-document
collection and measures indexing and search throughput.

```bash
cd benchmark
cargo run --release
```

For micro-benchmarks, use `cargo bench` in the crate of interest
(currently only `filters`).

## Project layout

```
crates/         # library crates
  api/          # HTTP handlers, DTOs, OpenAPI
  auth/         # master key, API keys, tenant tokens
  cache/        # LRU + TTL cache
  cluster/      # cluster skeleton (TODO)
  collections/  # collection metadata service
  config/       # TOML config, validation
  documents/    # document service (add, update, delete, get, list)
  errors/       # AppError and From impls
  facets/       # facet distribution
  filters/      # filter parser & evaluator
  hybrid/       # hybrid query fusion (RRF)
  highlighting/ # <em> highlight
  indexing/     # tokenization + inverted index + FST term dict
  metrics/      # Prometheus exporter
  models/       # shared data types
  replication/  # replication skeleton (TODO)
  scheduler/    # cron + interval jobs
  search/       # BM25, ranking, query parser
  security/     # rate limit, CORS, audit
  server/       # HTTP lifecycle, banner
  sharding/     # sharding skeleton (TODO)
  snapshots/    # tar+zstd snapshots
  storage/      # StorageBackend trait + redb
  synonyms/     # synonym map storage and lookup
  tasks/        # async task queue
  telemetry/    # tracing-subscriber setup
  typo/         # Damerau-Levenshtein
  utils/        # helpers (hash, random, time)
  validation/   # input validation + sanitization
  vector/       # embedding types + quantization
config/         # default.toml
docs/           # mdbook user guide (this site)
benchmark/      # standalone benchmark project
.github/        # CI + release + docs workflows
```

## Adding a new feature

1. **Decide the layer.** Filters belong in `filters/`, ranking tweaks
   in `search/`, persistence in `storage/`, etc. The crate boundaries
   exist to keep compile times low; honour them.
2. **Define the data type in `models`.** Public types live there so
   they can be shared across crates without circular deps.
3. **Write a failing unit test first.** Tests are colocated with code.
4. **Implement the feature.** No `unwrap` outside tests; no `unsafe`.
5. **Add an integration test** if the feature is exposed via HTTP.
6. **Update `docs/api.md`** if a new route was added, and
   `docs/configuration.md` if a new config key was added.
7. **Run `cargo fmt`, `cargo clippy`, and `cargo test` before
   opening a PR.**

## Adding a new crate

1. `mkdir crates/<name> && cd crates/<name>`
2. Copy a minimal `Cargo.toml` from a sibling crate; pin the workspace
   deps and inherit the `lints` table.
3. Add it to `[workspace.dependencies]` in the workspace `Cargo.toml` if
   other crates need to depend on it, otherwise just to the
   `[workspace] members` list (default).
4. Update `docs/architecture.md` to show the new crate in the diagram.

## Releasing

1. Bump versions: `cargo set-version --workspace 0.X.0` (manual edit
   acceptable).
2. Update `CHANGELOG.md` (chronological, newest first).
3. Tag the commit: `git tag v0.X.0`.
4. Push the tag; `.github/workflows/release.yml` cross-compiles for
   six targets and publishes a draft release.

## Common pitfalls

* **`unwrap` in non-test code** will fail clippy. Use `?`, `.expect("…")`
  with a justification, or convert the error via `From` into `AppError`.
* **Forgetting to invalidate the search cache** after a document write.
  See `crates/api/src/v1/documents.rs` for the pattern.
* **Using `println!` for logging.** Use `tracing::{info, warn, error}` so
  the structured-logging pipeline picks it up.
* **Adding a new env var** that doesn't go through `crates/config`. All
  configuration must be TOML-driven; env vars are an escape hatch only.
