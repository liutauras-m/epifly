# agent-gateway image. Build context: the repository root.
# The Tauri shell is outside the Cargo workspace, so nothing GUI-related is built here.

FROM rust:1.99.1-slim-trixie AS chef
RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config libssl-dev cmake clang curl \
    && rm -rf /var/lib/apt/lists/* \
    && cargo install cargo-chef --locked --version 0.1.78
WORKDIR /build

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --locked --recipe-path recipe.json -p agent-gateway
COPY . .
RUN cargo build --release --locked --bin agent-gateway

FROM debian:trixie-slim AS gateway
RUN apt-get update && apt-get install -y --no-install-recommends \
        libssl3t64 ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 epifly \
    && useradd --system --uid 10001 --gid epifly --no-create-home --shell /usr/sbin/nologin epifly \
    && mkdir -p /data && chown 10001:10001 /data
WORKDIR /app
COPY --from=builder /build/target/release/agent-gateway /app/agent-gateway
COPY --chown=10001:10001 apps/backend/capabilities/ /app/capabilities/

# The capabilities directory is writable by the gateway (state.json, manifests registered
# through the admin API); registrations live as long as the container, services re-register.
ENV PORT=8080 \
    CONUSAI_CAPABILITIES_DIR=/app/capabilities \
    REDB_PATH=/data/epifly.redb \
    EMBEDDING_CACHE_DIR=/data/embeddings

# /data is the only volume: redb and the embedding model cache.
VOLUME ["/data"]
USER 10001:10001
EXPOSE 8080
HEALTHCHECK --interval=15s --timeout=5s --retries=10 --start-period=120s \
    CMD curl -sf http://127.0.0.1:8080/health || exit 1
ENTRYPOINT ["/app/agent-gateway"]
