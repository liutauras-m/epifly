#!/usr/bin/env bash
# scripts/reindex.sh — Re-embed all workspace content and capability vectors
# into Qdrant collections for the active embedding model.
#
# Use after changing EMBEDDING_LOCAL_MODEL to migrate vectors to the new dims.
# The gateway uses per-dim collection names (e.g. capability_embeddings_d1024),
# so old and new collections coexist during migration.
#
# Usage:
#   ./scripts/reindex.sh [--dry-run] [--model multilingual-e5-large]
#
# Env vars (passed through to the gateway):
#   QDRANT_URL          default: http://localhost:6334
#   REDB_PATH           default: /data/conusai.redb
#   EMBEDDING_LOCAL_MODEL  default: multilingual-e5-large
#   EMBEDDING_CACHE_DIR default: (fastembed's own default)
#
# The script calls `POST /internal/admin/reindex` on the running gateway.
# If the gateway is not running, set CONUSAI_BACKEND_URL to point at it.

set -euo pipefail

DRY_RUN=0
MODEL="${EMBEDDING_LOCAL_MODEL:-multilingual-e5-large}"
BACKEND="${CONUSAI_BACKEND_URL:-http://localhost:8080}"
ADMIN_TOKEN="${PLATFORM_ADMIN_TOKEN:-}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) DRY_RUN=1; shift ;;
    --model) MODEL="$2"; shift 2 ;;
    *) echo "Unknown argument: $1" >&2; exit 1 ;;
  esac
done

echo "==> reindex.sh"
echo "    model:   $MODEL"
echo "    backend: $BACKEND"
echo "    dry-run: $DRY_RUN"
echo ""

if [[ -z "$ADMIN_TOKEN" ]]; then
  echo "WARN: PLATFORM_ADMIN_TOKEN not set — request may be rejected by the gateway" >&2
fi

PAYLOAD=$(printf '{"model":"%s","dry_run":%s}' "$MODEL" "$([ "$DRY_RUN" -eq 1 ] && echo true || echo false)")

echo "==> Triggering reindex..."
HTTP_STATUS=$(curl -s -o /tmp/reindex_response.json -w "%{http_code}" \
  -X POST "${BACKEND}/internal/admin/reindex" \
  -H "Content-Type: application/json" \
  -H "X-Platform-Admin-Token: ${ADMIN_TOKEN}" \
  -d "$PAYLOAD")

if [[ "$HTTP_STATUS" -eq 501 ]]; then
  echo ""
  echo "NOTE: The gateway returned 501 Not Implemented for /internal/admin/reindex."
  echo "This endpoint is a placeholder. To perform a manual reindex:"
  echo ""
  echo "  1. Stop the gateway."
  echo "  2. Delete the old Qdrant collections for the previous model's dims."
  echo "  3. Set EMBEDDING_LOCAL_MODEL=$MODEL and restart the gateway."
  echo "  4. The gateway will recreate collections at the correct dims on boot."
  echo "  5. Trigger re-embedding by posting each WorkspaceNode through the indexer:"
  echo "     curl -X POST $BACKEND/internal/rustfs/events -d '{\"type\":\"reindex_all\"}'"
  echo ""
  exit 0
fi

if [[ "$HTTP_STATUS" -lt 200 || "$HTTP_STATUS" -ge 300 ]]; then
  echo "ERROR: gateway returned HTTP $HTTP_STATUS" >&2
  cat /tmp/reindex_response.json >&2
  exit 1
fi

echo "==> Done (HTTP $HTTP_STATUS)"
cat /tmp/reindex_response.json
