<div align="center">

# AccelerateSearch

### *An open-source, Rust-native search engine built for speed, scale, and self-hosting.*

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Rust](https://img.shields.io/badge/Rust-2024-ed1c24.svg?logo=rust)](https://www.rust-lang.org/)
[![Edition](https://img.shields.io/badge/Edition-2024-orange.svg)](https://doc.rust-lang.org/edition-guide/)
[![Version](https://img.shields.io/badge/version-0.0.0-brightgreen.svg)](https://github.com/muhammad-fiaz/AccelerateSearch/releases)
[![GitHub stars](https://img.shields.io/github/stars/muhammad-fiaz/AccelerateSearch.svg)](https://github.com/muhammad-fiaz/AccelerateSearch/stargazers)
[![GitHub issues](https://img.shields.io/github/issues/muhammad-fiaz/AccelerateSearch.svg)](https://github.com/muhammad-fiaz/AccelerateSearch/issues)
[![Platforms](https://img.shields.io/badge/platforms-linux%20%7C%20macos%20%7C%20windows-lightgrey.svg)]()

</div>

---

> [!WARNING]
> **This project is currently in active development.** APIs, on-disk
> formats, configuration keys, and CLI flags may change without
> notice. Pin a specific commit or release tag if you need stability,
> and expect breaking changes between minor versions until `1.0.0`.

---

## What is AccelerateSearch?

AccelerateSearch is a **production-grade, self-hosted search engine** written in Rust. It speaks a clean REST API, ships as a single binary, and provides the developer experience of Meilisearch with the power of Elasticsearch, the simplicity of Typesense, and the vector capabilities of Qdrant, all in one Rust-native package.

It targets engineers who want to embed a search engine in their app, build it into a website, or wire it into an internal toolchain, without operating a JVM, an Elasticsearch cluster, or a managed service.

## Why AccelerateSearch?

- **Single binary** with no JVM, no service mesh, no sidecar, and no separate database.
- **Sub-10 ms p99 search** on commodity hardware for collections under 1 M documents.
- **First-class vector search** with HNSW indexing, cosine, dot, and Euclidean similarity.
- **Hybrid ranking** that fuses BM25 and vector signals with Reciprocal Rank Fusion.
- **Meilisearch-style developer experience** with tasks, settings, bulk ingestion, and scoped API keys.
- **Apache-2.0 licensed** for use in commercial or open-source projects without strings.

> [!NOTE]
> *Sub-second BM25 search, vector KNN, hybrid ranking, typo tolerance, synonyms, faceting, highlighting, a single binary that replaces your search stack.*
>
> *Built in Rust. Runs anywhere. Zero external services required.*

## Installation

Pick whichever path fits your workflow. All four methods install the
**same** binary; the difference is only in how the source gets onto
your machine and how the binary lands on your `PATH`.

### 1. Download a Pre-built Binary (recommended)

The fastest path. Grab the latest release for your platform from
[GitHub Releases](https://github.com/muhammad-fiaz/AccelerateSearch/releases):

| Platform | File |
|----------|------|
| Linux (x86_64)   | `accelerate-linux-amd64` |
| Linux (ARM64)    | `accelerate-linux-arm64` |
| macOS (Intel)    | `accelerate-macos-amd64` |
| macOS (Apple Silicon) | `accelerate-macos-arm64` |
| Windows          | `accelerate-windows-amd64.exe` |

```bash
# Linux / macOS
curl -L -o accelerate \
  https://github.com/muhammad-fiaz/AccelerateSearch/releases/latest/download/accelerate-$(uname -s | tr '[:upper:]' '[:lower:]')-amd64
chmod +x accelerate
sudo mv accelerate /usr/local/bin/accelerate
accelerate version

# Windows (PowerShell)
Invoke-WebRequest -Uri "https://github.com/muhammad-fiaz/AccelerateSearch/releases/latest/download/accelerate-windows-amd64.exe" `
  -OutFile "$env:ProgramFiles\accelerate.exe"
& "$env:ProgramFiles\accelerate.exe" version
```

### 2. Install with `cargo install` (from the Git repo)

If you already have the Rust toolchain installed, you can install
AccelerateSearch directly from the GitHub repository without cloning
it first:

```bash
# Latest commit on the default branch
cargo install --git https://github.com/muhammad-fiaz/AccelerateSearch --bin accelerate

# A specific tag (recommended for reproducible builds)
cargo install --git https://github.com/muhammad-fiaz/AccelerateSearch \
              --tag v0.0.0 \
              --bin accelerate

# A specific commit (for pinning in CI)
cargo install --git https://github.com/muhammad-fiaz/AccelerateSearch \
              --rev <commit-sha> \
              --bin accelerate
```

The compiled binary lands in `~/.cargo/bin/accelerate` (add
`~/.cargo/bin` to your `PATH` if it is not already).

### 3. Clone and Build from Source

Use this path if you want to hack on the code, run the test suite, or
produce a debug build.

```bash
# Clone the repository
git clone https://github.com/muhammad-fiaz/AccelerateSearch
cd AccelerateSearch

# Debug build (fast compile, slower runtime)
cargo build

# Release build (slow compile, fast runtime — the binary the
# release artefacts ship)
cargo build --release

# Optional: install the freshly built binary onto your PATH
cargo install --path . --bin accelerate

# Run the test suite
cargo test --workspace --no-fail-fast
```

After `cargo build --release` the binary is at
`target/release/accelerate` (or `accelerate.exe` on Windows). Move it
to a directory on your `PATH`:

```bash
# Linux / macOS
sudo cp target/release/accelerate /usr/local/bin/accelerate

# Windows
copy target\release\accelerate.exe C:\Windows\System32\accelerate.exe
```

### 4. Docker

Use the published image, or build it yourself from the included
`Dockerfile`:

```bash
# Pull (when published)
docker pull ghcr.io/muhammad-fiaz/accelerate:latest

# Or build locally
docker build -t accelerate:local .

# Run, persisting the data dir on the host
docker run --rm -p 7700:7700 \
  -v "$PWD/data:/data" \
  accelerate:local
```

The full `docker-compose.yml` is in the repository root for a
one-command bring-up.

## Quick Start

### Start the Server

```bash
# Start with default config (binds to http://localhost:7700)
accelerate start

# Start with custom options
accelerate start \
  --host 0.0.0.0 \
  --port 7700 \
  --data-dir /var/lib/accelerate \
  --log-level info \
  --master-key $(openssl rand -hex 32)
```

> [!TIP]
> The default bind address is **`localhost`** (loopback only). Pass
> `--host 0.0.0.0` to listen on every interface, or `--host
> 127.0.0.1` to keep the same loopback-only behaviour. The server
> exposes a full REST API under `/api/v1/`, ships a Swagger UI at
> `/swagger-ui/`, and prints a structured banner with version,
> environment, and data dir on startup.

## CLI Surface

```text
accelerate --help
accelerate start [--config FILE] [--host ADDR] [--port N]
                 [--data-dir DIR] [--master-key KEY] [--log-level LEVEL]
                 [--env ENV] [--no-color] [--quiet] [--no-banner]
                 [--no-console] [--no-file] [--disable-auth]
                 [--no-swagger] [--no-openapi]
accelerate version
accelerate health [--url URL]
accelerate snapshot create | list | info NAME | delete NAME | restore --path FILE
```

| Flag | Description |
| --- | --- |
| `--no-color` | Strip ANSI colour from every console surface. |
| `--quiet` | Silence every non-error log line and skip the banner. |
| `--no-banner` | Skip the ASCII art banner at startup. |
| `--no-console` | Disable console log output entirely. |
| `--no-file` | Disable the rotating file log appender. |
| `--no-swagger` | Disable the Swagger UI at `/swagger-ui`. |
| `--no-openapi` | Disable the OpenAPI spec at `/api-docs/openapi.json`. |

> [!WARNING]
> Every flag is also exposed as an `ACCELERATE_*` environment variable, and the TOML configuration can override defaults at the lowest precedence.

## Documentation

The user guide is an [mdbook](https://rust-lang.github.io/mdBook/) site
in [`docs/`](./docs/src) and is published to GitHub Pages at
<https://muhammad-fiaz.github.io/AccelerateSearch/>.

Build the docs locally with `cargo`:

```bash
# Install the toolchain helper (one-off)
cargo install mdbook --locked --version 0.4.43

# Build the mdbook user guide into docs/book/
(cd docs && mdbook build)

# Build the Rust API reference into target/doc/
cargo doc --no-deps --workspace --target-dir target

# Combine both into a single ./site/ directory ready for GitHub Pages
mkdir -p site
cp -R docs/book/. site/
mkdir -p site/rust-api
cp -R target/doc/. site/rust-api/
touch site/.nojekyll
```

The full reference (deployed to GitHub Pages):

- [`architecture.md`](./docs/architecture.md): module dependency graph and data flow.
- [`api.md`](./docs/api.md): full REST API reference.
- [`configuration.md`](./docs/configuration.md): every TOML key with type, default, and description.
- [`deployment.md`](./docs/deployment.md): Docker, systemd, and bare-metal deployment.
- [`development.md`](./docs/development.md): build, test, benchmark, contribute.
- [`rust-api.md`](./docs/rust-api.md): links to the generated
  `cargo doc` output.

> [!NOTE]
> The OpenAPI spec is served at runtime from `/api-docs/openapi.json`
> and rendered interactively at `/swagger-ui`.
>
> Every push to `main` rebuilds the docs site and deploys it to
> `gh-pages` via the
> [`.github/workflows/docs.yml`](./.github/workflows/docs.yml) workflow.
>
> The full architecture (crate dependency graph, data flow, storage
> layout) lives in [`docs/architecture.md`](./docs/architecture.md).

## Testing

```bash
cargo test --workspace
```

> [!TIP]
> All tests are inline in the source files (`#[cfg(test)]` modules). No separate `tests/` folder is used.

## Benchmarks

```bash
cargo bench --bench evaluate
```

The unified `benches/evaluate.rs` suite exercises every critical path: tokenisation, indexing, search, filter evaluation, faceting, highlighting, typo tolerance, vector KNN, hybrid RRF, collection CRUD, and HTTP round-trips. Results are written to `target/criterion/`.

## Contributing

See [`CONTRIBUTING.md`](./CONTRIBUTING.md) for development setup, code style, and the pull-request workflow. Issues and feature requests are tracked on GitHub.

## License

Copyright 2026 Muhammad Fiaz. Licensed under the [Apache License, Version 2.0](./LICENSE).

## Community & Support

- [Open an issue](https://github.com/muhammad-fiaz/AccelerateSearch/issues/new/choose)
- [Request a feature](https://github.com/muhammad-fiaz/AccelerateSearch/issues/new?template=feature_request.md)
- [Ask for help](https://github.com/muhammad-fiaz/AccelerateSearch/issues/new?template=help.md)
- [Star the repository](https://github.com/muhammad-fiaz/AccelerateSearch)
