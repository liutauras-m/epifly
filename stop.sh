#!/usr/bin/env bash
# =============================================================================
# ConusAI Platform — root stop script
#
# Usage:
#   ./stop.sh                  stop full Docker profile (default)
#   ./stop.sh infra            stop infra profile
#   ./stop.sh observability    stop observability profile
#   ./stop.sh full wipe        stop + remove all volumes, images, networks
#   ./stop.sh all              stop every profile
#   ./stop.sh all wipe         wipe everything
# =============================================================================
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

_usage() {
	cat <<'EOF'
Usage:
  ./stop.sh [profile] [wipe]

Profiles (default: full):
  full           Full Docker stack
  infra          Qdrant + RustFS
  observability  Jaeger + OTel collector
  all            Stop every profile

Options:
  wipe, --wipe, -w    Remove volumes, images, and networks (clean slate)

Examples:
  ./stop.sh                  # stop full profile
  ./stop.sh infra            # stop infra profile
  ./stop.sh full wipe        # stop + wipe all data
  ./stop.sh all wipe         # nuclear: destroy everything
EOF
}

PROFILE="full"
WIPE=false

for arg in "$@"; do
	case "$arg" in
		full|infra|observability|all)
			PROFILE="$arg"
			;;
		wipe|--wipe|-w|ti)
			WIPE=true
			;;
		-h|--help|help)
			_usage
			exit 0
			;;
		*)
			echo "❌  Unknown argument: '$arg'"
			_usage
			exit 1
			;;
	esac
done

echo "⏹   Stopping ConusAI Platform…"

# =============================================================================
# 1. Kill host-process gateway
# =============================================================================
if pkill -f "target/debug/agent-gateway" 2>/dev/null; then
	echo "✅  Local gateway process stopped"
fi

# =============================================================================
# 2. Kill Tauri processes (desktop + mobile dev)
#    - "tauri dev" is the CLI wrapper process
#    - The compiled app bundle name is "ConusAI Browser" (from tauri.conf.json)
#    - apps/browser-shell catches any pnpm/vite child in that directory
# =============================================================================
pkill -f "tauri dev"            2>/dev/null || true
pkill -f "tauri ios dev"        2>/dev/null || true
pkill -f "tauri android dev"    2>/dev/null || true
pkill -f "ConusAI Browser"      2>/dev/null || true   # macOS app bundle
pkill -f "com.conusai.browser"  2>/dev/null || true   # bundle identifier
pkill -f "apps/browser-shell"   2>/dev/null || true   # Vite dev under shell dir

# =============================================================================
# 3. Kill SvelteKit / Vite dev servers
#    Filter by directory path to avoid killing unrelated projects.
# =============================================================================
pkill -f "$ROOT_DIR/apps/web/node_modules/.bin/vite" 2>/dev/null || true
pkill -f "$ROOT_DIR/apps/browser-shell/node_modules/.bin/vite" 2>/dev/null || true

# Brief pause so OS can clean up sockets before Docker stops containers.
sleep 0.5

# =============================================================================
# 4. Stop Docker services
# =============================================================================
_stop_profile() {
	local p="$1"
	if [[ "$WIPE" == true ]]; then
		echo "🗑   Wiping Docker profile: $p"
		docker compose --profile "$p" down --volumes --rmi all --remove-orphans
	else
		echo "▶   Stopping Docker profile: $p"
		docker compose --profile "$p" down
	fi
}

if [[ "$PROFILE" == "all" ]]; then
	for p in full infra observability; do
		_stop_profile "$p" 2>/dev/null || true
	done
else
	_stop_profile "$PROFILE"
fi

echo "✅  Platform stopped"
[[ "$WIPE" == true ]] && echo "    (volumes and images removed)"
