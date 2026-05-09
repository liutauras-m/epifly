#!/usr/bin/env bash
# Root shim — delegates to apps/backend/stop.sh
set -euo pipefail
exec "$(dirname "$0")/apps/backend/stop.sh" "$@"
