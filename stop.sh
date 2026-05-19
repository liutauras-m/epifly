#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Always kill any host-process gateway first.
pkill -f "target/debug/agent-gateway" 2>/dev/null && echo "✅ Local gateway process stopped" || true

# local mode: no Docker gateway container to stop, Docker infra still brought down.
if [ "${1:-}" = "local" ]; then
  shift || true
  exec "$ROOT_DIR/apps/backend/stop.sh" "full" "$@"
fi

exec "$ROOT_DIR/apps/backend/stop.sh" "$@"
