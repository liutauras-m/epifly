#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  ./start.sh [profile]
  ./start.sh start [profile]
  ./start.sh stop [profile]

Commands:
  start          Start services for a profile (default)
  stop           Stop services for a profile

Profiles:
  infra          Qdrant + RustFS (+ rustfs-init)
  full           Full stack (gateway + infra + observability)
  observability  Jaeger + OTel collector
EOF
}

ACTION="start"
PROFILE="infra"   # infra | full | observability

if [ "${1:-}" = "start" ] || [ "${1:-}" = "stop" ]; then
  ACTION="$1"
  PROFILE="${2:-infra}"
elif [ -n "${1:-}" ]; then
  # Backward compatibility: ./start.sh full
  PROFILE="$1"
fi

case "$PROFILE" in
  infra|full|observability)
    ;;
  -h|--help|help)
    usage
    exit 0
    ;;
  *)
    echo "❌ Invalid profile: $PROFILE"
    usage
    exit 1
    ;;
esac

if [ "$ACTION" = "stop" ]; then
  echo "🛑 ConusAI Platform — stopping profile: $PROFILE"
  docker compose --profile "$PROFILE" down
  echo "✅ Profile stopped"
  exit 0
fi

echo "🚀 ConusAI Platform — starting profile: $PROFILE"

# ── Environment ───────────────────────────────────────────────────────────────
if [ ! -f .env ]; then
  if [ -f .env.local ]; then
    echo "⚠️  No .env found — copying from .env.local"
    cp .env.local .env
  elif [ -f .env.example ]; then
    echo "⚠️  No .env found — copying from .env.example"
    cp .env.example .env
  else
    echo "⚠️  No .env, .env.local, or .env.example found — creating empty .env"
    : > .env
  fi
fi

# ── Infrastructure ────────────────────────────────────────────────────────────
echo "▶ Starting infrastructure services..."
docker compose --profile "$PROFILE" up -d

# ── Wait for Qdrant ──────────────────────────────────────────────────────────
echo "⏳ Waiting for Qdrant..."
until curl -sf http://localhost:6333/healthz > /dev/null 2>&1; do sleep 1; done
echo "✅ Qdrant ready"

# ── Wait for RustFS ───────────────────────────────────────────────────────────
echo "⏳ Waiting for RustFS..."
until curl -sf http://localhost:9000/minio/health/live > /dev/null 2>&1; do sleep 1; done
echo "✅ RustFS ready"

# ── Build agent-gateway (if running full profile) ─────────────────────────────
if [ "$PROFILE" = "full" ]; then
  echo "▶ Building agent-gateway..."
  cargo build --release --bin agent-gateway --features agent-gateway/local-embeddings
  echo "✅ Build complete — gateway running in Docker"
fi

# ── Capability discovery info ─────────────────────────────────────────────────
CAP_COUNT=$(find capabilities -maxdepth 2 -name "capability.toml" 2>/dev/null | wc -l | tr -d ' ')
echo "📦 Capabilities discovered: $CAP_COUNT"

echo ""
echo "ConusAI Platform is ready."
echo "  Gateway:   http://localhost:8080"
echo "  Foundry UI http://localhost:8080/login"
echo "  Qdrant:    http://localhost:6333"
echo "  RustFS:    http://localhost:9001"
