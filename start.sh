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
  infra          Qdrant + MinIO (+ minio-init)
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
  echo "⚠️  No .env found — copying from .env.example"
  cp .env.example .env
fi

# ── Infrastructure ────────────────────────────────────────────────────────────
echo "▶ Starting infrastructure services..."
docker compose --profile "$PROFILE" up -d --wait

# ── Wait for Qdrant ───────────────────────────────────────────────────────────
echo "⏳ Waiting for Qdrant..."
until curl -sf http://localhost:6333/healthz > /dev/null 2>&1; do sleep 1; done
echo "✅ Qdrant ready"

# ── Build agent-gateway (if running full profile) ─────────────────────────────
if [ "$PROFILE" = "full" ]; then
  echo "▶ Building agent-gateway..."
  cargo build --release --bin agent-gateway
  echo "✅ Build complete — gateway running in Docker"
fi

# ── Capability discovery info ─────────────────────────────────────────────────
CAP_COUNT=$(find capabilities -maxdepth 2 -name "capability.yaml" 2>/dev/null | wc -l | tr -d ' ')
echo "📦 Capabilities discovered: $CAP_COUNT"

echo ""
echo "ConusAI Platform is ready."
echo "  Gateway:   http://localhost:8080"
echo "  Foundry UI http://localhost:8080/login"
echo "  Qdrant:    http://localhost:6333"
echo "  MinIO:     http://localhost:9001"
