<div align="center">

# ⚡ AccelerateSearch

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

## ✨ What is AccelerateSearch?

AccelerateSearch is a **production-grade, self-hosted search engine** written in Rust. It speaks a clean REST API, ships as a single binary, and provides the developer experience of Meilisearch with the power of Elasticsearch, the simplicity of Typesense, and the vector capabilities of Qdrant — all in one Rust-native package.

It targets engineers who want to embed a search engine in their app, build it into a website, or wire it into an internal toolchain — without operating a JVM, an Elasticsearch cluster, or a managed service.

## 🎯 Why AccelerateSearch?

- **Single binary** — no JVM, no service mesh, no sidecar, no separate database.
- **Sub-10 ms p99 search** on commodity hardware for collections under 1 M documents.
- **First-class vector search** with HNSW indexing, cosine / dot / Euclidean similarity.
- **Hybrid ranking** that fuses BM25 and vector signals with Reciprocal Rank Fusion.
- **Meilisearch-style developer experience** — tasks, settings, bulk ingestion, scoped API keys.
- **Apache-2.0 licensed** — use it in commercial or open-source projects without strings.

> [!NOTE]
> *Sub-second BM25 search, vector KNN, hybrid ranking, typo tolerance, synonyms, faceting, highlighting — a single binary that replaces your search stack.*
>
> *Built in Rust. Runs anywhere. Zero external services required.*

## 🚀 Quick Start

### Download Pre-built Binary

Download the latest release for your platform from [GitHub Releases](https://github.com/muhammad-fiaz/AccelerateSearch/releases):

| Platform | File |
|----------|------|
| Linux (x86_64) | `accelerate-linux-amd64` |
| Linux (ARM64) | `accelerate-linux-arm64` |
| macOS (Intel) | `accelerate-macos-amd64` |
| macOS (Apple Silicon) | `accelerate-macos-arm64` |
| Windows | `accelerate-windows-amd64.exe` |

```bash
# Linux/macOS - Make executable and move to PATH
chmod +x accelerate-linux-amd64
sudo mv accelerate-linux-amd64 /usr/local/bin/accelerate

# Windows - Move to PATH
move accelerate-windows-amd64.exe C:\Windows\System32\accelerate.exe
```

### Start the Server

```bash
# Start with default config
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
> The server starts on `http://localhost:7700`, exposes a full REST API under `/api/v1/`, ships a Swagger UI at `/swagger-ui`, and prints a structured banner with version, environment, and data dir.

## 🔧 CLI Surface

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

## 📚 Documentation

The full reference lives in [`docs/`](./docs):

- [`docs/architecture.md`](./docs/architecture.md) — module dependency graph and data flow.
- [`docs/api.md`](./docs/api.md) — full REST API reference.
- [`docs/configuration.md`](./docs/configuration.md) — every TOML key with type, default, and description.
- [`docs/deployment.md`](./docs/deployment.md) — Docker, systemd, and bare-metal deployment.
- [`docs/development.md`](./docs/development.md) — build, test, benchmark, contribute.

> [!NOTE]
> The OpenAPI spec is served at runtime from `/api-docs/openapi.json` and rendered interactively at `/swagger-ui`.

## 🏗️ Architecture

```
            ┌──────────────────────┐
            │      REST API        │   actix-web + utoipa
            └──────────┬───────────┘
                       │
            ┌──────────▼───────────┐
            │      Services        │   auth, tasks, snapshots
            └──────────┬───────────┘
                       │
   ┌──────────┬────────┼────────┬──────────┐
   ▼          ▼        ▼        ▼          ▼
┌──────┐  ┌──────┐ ┌──────┐ ┌──────┐  ┌────────┐
│Index │  │Search│ │Vector│ │Filter│  │Storage │
│ing   │  │      │ │KNN   │ │      │  │ redb   │
└──────┘  └──────┘ └──────┘ └──────┘  └────────┘
```

The workspace is split into 30 modular crates, each with a clear single responsibility. The binary is a thin shell that wires them together and serves them over HTTP.

## 🧪 Testing

```bash
cargo test --workspace
```

> [!TIP]
> All tests are inline in the source files (`#[cfg(test)]` modules). No separate `tests/` folder is used.

## ⚡ Benchmarks

```bash
cargo bench --bench evaluate
```

The unified `benches/evaluate.rs` suite exercises every critical path: tokenisation, indexing, search, filter evaluation, faceting, highlighting, typo tolerance, vector KNN, hybrid RRF, collection CRUD, and HTTP round-trips. Results are written to `target/criterion/`.

## 🤝 Contributing

See [`CONTRIBUTING.md`](./CONTRIBUTING.md) for development setup, code style, and the pull-request workflow. Issues and feature requests are tracked on GitHub.

## 📄 License

Copyright © 2026 Muhammad Fiaz. Licensed under the [Apache License, Version 2.0](./LICENSE).

## 💬 Community & Support

- 🐛 [Open an issue](https://github.com/muhammad-fiaz/AccelerateSearch/issues/new/choose)
- 💡 [Request a feature](https://github.com/muhammad-fiaz/AccelerateSearch/issues/new?template=feature_request.md)
- 🆘 [Ask for help](https://github.com/muhammad-fiaz/AccelerateSearch/issues/new?template=help.md)
- 🌟 [Star the repository](https://github.com/muhammad-fiaz/AccelerateSearch)
