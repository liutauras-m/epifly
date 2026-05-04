#!/usr/bin/env bash
# Root shim — delegates to apps/backend/start.sh
set -euo pipefail
exec "$(dirname "$0")/apps/backend/start.sh" "$@"
