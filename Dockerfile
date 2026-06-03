# Multi-stage build for AccelerateSearch.
# Stage 1: build the binary inside a thin builder image.
# Stage 2: copy the binary into a distroless base.

FROM rust:1.85-bookworm AS builder

WORKDIR /build

# Cache dependencies separately for faster incremental builds.
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY benchmark ./benchmark
COPY config ./config

# Pre-warm the target dir with a stub main so deps build once.
RUN mkdir -p src \
    && echo "fn main() {}" > src/main.rs \
    && cargo build --release --bin accelerate \
    && rm -rf src

# Now copy the real source and rebuild only what changed.
COPY src ./src
RUN touch src/main.rs \
    && cargo build --release --bin accelerate

# ----------------------------------------------------------------------------

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -r -u 1000 -m -d /data -s /usr/sbin/nologin accelerate

WORKDIR /app
COPY --from=builder /build/target/release/accelerate /usr/local/bin/accelerate
COPY config/default.toml /app/config/default.toml

USER accelerate
WORKDIR /data

ENV ACCELERATE_CONFIG=/app/config/default.toml
ENV ACCELERATE_DATA_DIR=/data
ENV RUST_LOG=accelerate=info,actix_web=info

EXPOSE 7700

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ["/usr/local/bin/accelerate", "--healthcheck"]

ENTRYPOINT ["/usr/local/bin/accelerate"]
