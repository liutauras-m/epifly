# ── Stage 1: cargo-chef planner ───────────────────────────────────────────────
FROM rust:1.95-slim AS planner

WORKDIR /build

RUN cargo install cargo-chef --locked

COPY . .

# Remove the Tauri browser-shell from the workspace for the Docker/Linux build.
# Tauri on Linux pulls in webkit2gtk → glib which are GUI libraries not needed
# by the backend. The browser-shell is a native macOS/iOS desktop app and is
# never deployed in the container.
RUN sed -i '/"apps\/browser-shell\/src-tauri"/d' Cargo.toml

RUN cargo chef prepare --recipe-path recipe.json

# ── Stage 2: cargo-chef cook (dependency cache layer) ─────────────────────────
FROM rust:1.95-slim AS cacher

WORKDIR /build

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    cmake \
    clang \
    curl \
    && rm -rf /var/lib/apt/lists/*

RUN cargo install cargo-chef --locked

COPY --from=planner /build/recipe.json recipe.json

RUN cargo chef cook --release --recipe-path recipe.json --features agent-gateway/local-embeddings

# ── Stage 3: builder ──────────────────────────────────────────────────────────
FROM rust:1.95-slim AS builder

WORKDIR /build

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    cmake \
    clang \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Restore pre-built deps from cacher
COPY --from=cacher /build/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo

COPY . .

# Apply the same workspace patch as the planner stage
RUN sed -i '/"apps\/browser-shell\/src-tauri"/d' Cargo.toml

RUN cargo build --release --bin agent-gateway --features agent-gateway/local-embeddings

# ── Stage 4: runtime ──────────────────────────────────────────────────────────
FROM ubuntu:24.04 AS gateway

RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl3 \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /build/target/release/agent-gateway /app/agent-gateway
COPY apps/backend/capabilities/ /app/capabilities/

ENV CONUSAI_SERVER__HOST=0.0.0.0
ENV CONUSAI_SERVER__PORT=8080
ENV CONUSAI_CAPABILITIES_DIR=/app/capabilities

EXPOSE 8080

HEALTHCHECK --interval=15s --timeout=5s --retries=3 \
    CMD curl -sf http://localhost:8080/health || exit 1

ENTRYPOINT ["/app/agent-gateway"]
