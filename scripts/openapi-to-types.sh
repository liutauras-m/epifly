#!/usr/bin/env bash
# Regenerate packages/types/src/openapi.d.ts from the gateway's OpenAPI document.
# `agent-gateway --dump-openapi` prints the document without starting the server.
set -euo pipefail
cd "$(dirname "$0")/.."
OUT=packages/types/src/openapi.d.ts
SPEC=$(mktemp --suffix=.json)
trap 'rm -f "$SPEC"' EXIT
cargo run --locked -q -p agent-gateway -- --dump-openapi > "$SPEC"
pnpm openapi-typescript "$SPEC" -o "$OUT"
echo "Generated $OUT"
