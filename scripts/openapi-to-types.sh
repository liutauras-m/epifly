#!/usr/bin/env bash
set -euo pipefail

PORT=${CONUSAI_TYPES_PORT:-8088}
OUT=packages/types/src/openapi.d.ts

# Start the gateway in test mode, wait for it, generate, kill it.
CONUSAI_TEST_MODE=1 CONUSAI_SERVER__PORT="${PORT}" cargo run -p agent-gateway &
GW_PID=$!

cleanup() { kill "$GW_PID" 2>/dev/null || true; }
trap cleanup EXIT

OPENAPI_URL="http://localhost:${PORT}/openapi.json"

echo "Waiting for OpenAPI on :${PORT}…"
for i in $(seq 1 30); do
  if curl -sf "${OPENAPI_URL}" >/dev/null 2>&1; then
    echo "OpenAPI ready."
    break
  fi
  sleep 1
  if [ "$i" = 30 ]; then
    echo "OpenAPI did not become ready in time." >&2
    exit 1
  fi
done

pnpm --filter @conusai/types exec openapi-typescript "$OPENAPI_URL" -o "$OUT"
echo "Generated $OUT"
