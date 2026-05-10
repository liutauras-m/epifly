#!/usr/bin/env bash
set -euo pipefail

PORT=${CONUSAI_TYPES_PORT:-8088}
OUT=packages/types/src/openapi.d.ts

# Start the gateway in test mode, wait for it, generate, kill it.
CONUSAI_TEST_MODE=1 cargo run -p agent-gateway &
GW_PID=$!

cleanup() { kill "$GW_PID" 2>/dev/null || true; }
trap cleanup EXIT

echo "Waiting for gateway on :${PORT}…"
for i in $(seq 1 30); do
  if curl -sf "http://localhost:${PORT}/health" >/dev/null 2>&1; then
    echo "Gateway ready."
    break
  fi
  sleep 1
  if [ "$i" = 30 ]; then
    echo "Gateway did not start in time." >&2
    exit 1
  fi
done

pnpm openapi-typescript "http://localhost:${PORT}/api-docs/openapi.json" -o "$OUT"
echo "Generated $OUT"
