#!/usr/bin/env bash
# scripts/dump-routes.sh — Generate the gateway route table as Markdown.
#
# Usage:
#   ./scripts/dump-routes.sh [--out docs/_routes.generated.md]
#
# The generated file is compared against docs/_routes.expected.md by
# `make verify-routes-doc`. Update _routes.expected.md after intentional
# route changes: cp docs/_routes.generated.md docs/_routes.expected.md

set -euo pipefail

OUT="${1:-docs/_routes.generated.md}"

echo "==> Building agent-gateway..."
cargo build --bin agent-gateway -q 2>&1

echo "==> Dumping routes to $OUT..."
cargo run --bin agent-gateway -q -- --dump-routes > "$OUT" 2>/dev/null

echo "==> Written: $OUT"
echo "    $(grep -c '^\| \`' "$OUT" || true) route entries"
