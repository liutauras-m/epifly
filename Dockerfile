# ── Stage 1: Build ────────────────────────────────────────────────────────────
FROM rust:1.95-slim AS builder

WORKDIR /build

# Install C dependencies for ring, native-tls, wasmtime
RUN apt-get update && apt-get install -y \
    pkg-config libssl-dev cmake clang \
    && rm -rf /var/lib/apt/lists/*

# Cache dependencies: copy manifests first
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates/common/Cargo.toml        crates/common/
COPY crates/agent-core/Cargo.toml    crates/agent-core/
COPY crates/agent-gateway/Cargo.toml crates/agent-gateway/
COPY crates/invoice-demo/Cargo.toml  crates/invoice-demo/
COPY evals/Cargo.toml                evals/

# Stub src files so cargo can build deps without real code
RUN mkdir -p crates/common/src crates/agent-core/src crates/agent-gateway/src \
             crates/invoice-demo/src evals/src \
    && echo "fn main(){}" > crates/agent-gateway/src/main.rs \
    && echo "fn main(){}" > crates/invoice-demo/src/main.rs \
    && echo "fn main(){}" > evals/src/main.rs \
    && echo "pub fn stub(){}" > crates/common/src/lib.rs \
    && echo "pub fn stub(){}" > crates/agent-core/src/lib.rs

RUN cargo build --release --bin agent-gateway 2>&1 || true

# Now copy real source and rebuild
COPY crates/ crates/
COPY evals/  evals/

RUN touch crates/*/src/*.rs evals/src/*.rs 2>/dev/null || true
RUN cargo build --release --bin agent-gateway

# ── Stage 2: Runtime ──────────────────────────────────────────────────────────
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
