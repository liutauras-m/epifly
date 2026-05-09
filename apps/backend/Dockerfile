# ── Stage 1: cargo-chef planner ───────────────────────────────────────────────
FROM rust:1.88-slim AS planner

WORKDIR /build

RUN cargo install cargo-chef --locked

COPY . .

RUN cargo chef prepare --recipe-path recipe.json

# ── Stage 2: cargo-chef cook (dependency cache layer) ─────────────────────────
FROM rust:1.88-slim AS cacher

WORKDIR /build

RUN apt-get update && apt-get install -y \
    pkg-config libssl-dev cmake clang curl \
    && rm -rf /var/lib/apt/lists/*

RUN cargo install cargo-chef --locked

COPY --from=planner /build/recipe.json recipe.json

RUN cargo chef cook --release --recipe-path recipe.json

# ── Stage 3: build ────────────────────────────────────────────────────────────
FROM rust:1.88-slim AS builder

WORKDIR /build

RUN apt-get update && apt-get install -y \
    pkg-config libssl-dev cmake clang curl \
    && rm -rf /var/lib/apt/lists/*

# Restore pre-built deps from cacher
COPY --from=cacher /build/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo

COPY . .

RUN cargo build --release --bin agent-gateway

# ── Stage 4: runtime ──────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS gateway

RUN apt-get update && apt-get install -y \
    libssl3 ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /build/target/release/agent-gateway /app/agent-gateway
COPY capabilities/ /app/capabilities/

ENV CONUSAI_SERVER__HOST=0.0.0.0
ENV CONUSAI_SERVER__PORT=8080
ENV CONUSAI_CAPABILITIES_DIR=/app/capabilities

EXPOSE 8080

HEALTHCHECK --interval=15s --timeout=5s --retries=3 \
    CMD curl -sf http://localhost:8080/health || exit 1

ENTRYPOINT ["/app/agent-gateway"]
