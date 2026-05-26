#!/usr/bin/env bash
# =============================================================================
# ConusAI Platform — root startup script
#
# Usage:
#   ./start.sh                   # full Docker stack (default)
#   ./start.sh full              # full Docker stack
#   ./start.sh infra             # Qdrant + RustFS only
#   ./start.sh observability     # Jaeger + OTel only
#   ./start.sh local             # Docker infra + local compiled gateway
#   ./start.sh web               # local + SvelteKit dev server (:5173)
#   ./start.sh tauri             # local + Tauri desktop (macOS / Linux)
#   ./start.sh tauri ios         # local + Tauri iOS Simulator
#   ./start.sh tauri android     # local + Tauri Android emulator
#   ./start.sh stop [profile] [wipe]
#   ./start.sh -h
#
# .env.local is always sourced first; variables already in the shell
# environment take precedence over defaults set with ${VAR:=value}.
# =============================================================================
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BACKEND_START="$ROOT_DIR/apps/backend/start.sh"
LOCAL_GATEWAY_BIN="$ROOT_DIR/target/debug/agent-gateway"
BROWSER_SHELL_DIR="$ROOT_DIR/apps/browser-shell"

# PIDs of background processes started by this script, killed on exit.
declare -a PIDS=()

# =============================================================================
# Environment
# =============================================================================

# Load .env.local once, right here, before anything else runs.
# `set -a` exports every variable that is assigned; `set +a` restores default.
# Variables already present in the shell are not overwritten (set -a only
# exports, it does not force-overwrite existing vars).
_load_env() {
	local env_file="$ROOT_DIR/.env.local"
	if [[ -f "$env_file" ]]; then
		set -a
		# shellcheck source=/dev/null
		source "$env_file"
		set +a
		echo "ℹ️   Loaded $env_file"
	else
		echo "⚠️   No .env.local found — using environment defaults."
		echo "    Copy .env.example → .env.local and add ANTHROPIC_API_KEY."
	fi
}
_load_env

# =============================================================================
# Lago RSA key
# =============================================================================

_ensure_lago_rsa_key() {
	# Already present in the environment (sourced from .env.local) — nothing to do.
	if [[ -n "${LAGO_RSA_PRIVATE_KEY:-}" ]]; then
		return 0
	fi

	local env_file="$ROOT_DIR/.env.local"
	LAGO_RSA_PRIVATE_KEY="$(openssl genrsa 2048 2>/dev/null | base64 | tr -d '\n')"
	export LAGO_RSA_PRIVATE_KEY

	# Append only if the key is not already recorded in the file (idempotent).
	if ! grep -q "^LAGO_RSA_PRIVATE_KEY=" "$env_file" 2>/dev/null; then
		echo "LAGO_RSA_PRIVATE_KEY=$LAGO_RSA_PRIVATE_KEY" >> "$env_file"
		echo "ℹ️   Generated LAGO_RSA_PRIVATE_KEY → $env_file"
	fi
}

# =============================================================================
# HTTP health polling
# =============================================================================

# wait_http <url> <http-code-regex> [timeout-secs=90]
_wait_http() {
	local url="$1"
	local pattern="$2"
	local timeout="${3:-90}"
	local start
	start="$(date +%s)"

	while true; do
		local code
		code="$(curl -s -o /dev/null -w '%{http_code}' "$url" 2>/dev/null || echo 000)"
		[[ "$code" =~ $pattern ]] && return 0
		if (( $(date +%s) - start >= timeout )); then
			echo "❌  Timeout waiting for $url  (last HTTP $code)"
			return 1
		fi
		sleep 2
	done
}

# =============================================================================
# Cleanup trap — kills all tracked background processes on exit / Ctrl-C
# =============================================================================

_cleanup() {
	local exit_code=$?
	echo ""
	if [[ ${#PIDS[@]} -gt 0 ]]; then
		echo "⏹   Stopping background processes…"
		# shellcheck disable=SC2068
		kill ${PIDS[@]} 2>/dev/null || true
	fi
	exit "$exit_code"
}
trap _cleanup EXIT INT TERM

# =============================================================================
# Docker helpers
# =============================================================================

_report_unhealthy() {
	local unhealthy
	unhealthy="$(docker ps --filter health=unhealthy --format '{{.Names}}' | grep '^conusai-' || true)"
	if [[ -n "$unhealthy" ]]; then
		echo "❌  Unhealthy containers:"
		printf '%s\n' "$unhealthy"
		return 1
	fi
	return 0
}

# =============================================================================
# Mode: Docker profile  (full | infra | observability)
# =============================================================================

_start_docker_profile() {
	local profile="$1"

	_ensure_lago_rsa_key

	echo "▶  Starting Docker profile: $profile"
	if ! "$BACKEND_START" "$profile"; then
		echo "⚠️   Initial start failed — retrying…"
		docker compose --profile "$profile" up -d
		docker compose up -d --force-recreate lago-api lago-worker 2>/dev/null || true
		docker compose restart zitadel 2>/dev/null || true
	fi

	if [[ "$profile" == "full" ]]; then
		_wait_http "http://localhost:8080/health" '^200$'           120
		_wait_http "http://localhost:3000/login"  '^(200|30[1278])$' 120
		_wait_http "http://localhost:9001/login"  '^(200|403)$'      120
		_report_unhealthy
	fi

	echo "✅  Profile $profile is up"
	_print_docker_urls "$profile"
}

_print_docker_urls() {
	local profile="$1"
	echo ""
	echo "  Gateway:   http://localhost:8080"
	[[ "$profile" == "full" ]] && echo "  Web UI:    http://localhost:3000"
	echo "  Qdrant:    http://localhost:6333"
	echo "  RustFS:    http://localhost:9001"
	echo "  Lago:      http://localhost:3010"
	echo "  Zitadel:   http://localhost:8085"
	echo ""
}

# =============================================================================
# Mode: local gateway  (shared by local / web / tauri)
# =============================================================================

# Sets up Docker infra + starts the compiled gateway binary in the background.
# Populates all required env vars from .env.local (loaded above) with sane
# dev-mode fallbacks for anything not set.
_start_infra_and_gateway() {
	if [[ ! -x "$LOCAL_GATEWAY_BIN" ]]; then
		echo "❌  Gateway binary not found: $LOCAL_GATEWAY_BIN"
		echo "    Build it first:"
		echo "      cargo build --bin agent-gateway"
		echo "    or with local embeddings:"
		echo "      cargo build --bin agent-gateway --features agent-gateway/local-embeddings"
		exit 1
	fi

	# Kill any leftover gateway from a previous run.
	pkill -f "target/debug/agent-gateway" 2>/dev/null || true
	sleep 0.3

	_ensure_lago_rsa_key

	# ── Docker infra (no agent-gateway container, no current-time) ─────────
	echo "▶  Starting infrastructure (Docker)…"
	docker compose up -d --no-build \
		postgres redis zitadel lago-api lago-worker \
		qdrant rustfs-perms rustfs

	_wait_http "http://localhost:6333/healthz" '^200$'      60 && echo "✅  Qdrant ready"
	_wait_http "http://localhost:9000"          '^(200|403)$' 60 && echo "✅  RustFS ready"

	# ── Gateway env — prefer .env.local values, fall back to dev defaults ──
	# NOTE: ${VAR:=default} only assigns if VAR is unset or empty.
	#       Because _load_env ran at the top, any key in .env.local is already
	#       exported and will NOT be overwritten here.
	: "${QDRANT_URL:=http://localhost:6334}"
	: "${S3_ENDPOINT:=http://localhost:9000}"
	: "${S3_BUCKET:=workspace}"
	: "${AWS_ACCESS_KEY_ID:=rustfsadmin}"
	: "${AWS_SECRET_ACCESS_KEY:=rustfsadmin}"
	: "${REDB_PATH:=/tmp/conusai-local-gw.redb}"
	: "${UI_SESSION_KEY:=conusai-foundry-dev-secret-change-me-32b}"
	if [[ "${CONUSAI_TEST_MODE:-0}" == "1" ]]; then
		unset JWT_SECRET
	else
		: "${JWT_SECRET:=change-me-in-production}"
		export JWT_SECRET
	fi
	# Tauri origins (tauri://localhost + https://tauri.localhost) are required
	# for the Tauri webview's CSP.  :5174 is the browser-shell Vite dev port.
	: "${WEB_ORIGIN:=http://localhost:3000,http://localhost:5173,http://localhost:5174,https://tauri.localhost,tauri://localhost}"
	: "${EMBEDDING_BACKEND:=local}"
	: "${CONUSAI_CAPABILITIES_DIR:=./capabilities}"
	: "${RUST_LOG:=info}"

	export QDRANT_URL S3_ENDPOINT S3_BUCKET AWS_ACCESS_KEY_ID AWS_SECRET_ACCESS_KEY
	export REDB_PATH UI_SESSION_KEY WEB_ORIGIN
	export EMBEDDING_BACKEND CONUSAI_CAPABILITIES_DIR RUST_LOG

	echo "▶  Starting local gateway on :8080…"
	pushd "$ROOT_DIR/apps/backend" > /dev/null
	"$LOCAL_GATEWAY_BIN" &
	PIDS+=($!)
	popd > /dev/null

	if [[ "${CONUSAI_TEST_MODE:-0}" == "1" ]]; then
		echo "⏳  Waiting for gateway (test mode uses in-memory stores; embeddings disabled)…"
		_wait_http "http://localhost:8080/health" '^(200|503)$' 30
		echo "✅  Gateway ready (test mode)"
		return 0
	fi

	# fastembed downloads its model (~50 MB) on first run; 90 s is generous.
	echo "⏳  Waiting for gateway (embeddings may load on first run, ~90 s)…"
	if _wait_http "http://localhost:8080/healthz/embeddings" '^200$' 90; then
		echo "✅  Gateway ready"
		# Surface OTEL exporter if configured
		if [[ -n "${OTEL_EXPORTER_OTLP_ENDPOINT:-}" ]]; then
			echo "ℹ️   OTEL exporter: $OTEL_EXPORTER_OTLP_ENDPOINT"
		fi
	else
		echo "⚠️   Gateway embeddings endpoint not ready — check gateway log above"
	fi
}

# =============================================================================
# Mode: local  (gateway binary only, no frontend)
# =============================================================================

_start_local() {
	_start_infra_and_gateway
	echo ""
	echo "  Gateway: http://localhost:8080"
	echo "  Qdrant:  http://localhost:6333"
	echo "  RustFS:  http://localhost:9001"
	echo ""
	echo "  Press Ctrl-C to stop all services."
	echo ""
	# Block here; Ctrl-C triggers _cleanup which kills gateway PID.
	wait "${PIDS[@]}"
}

# =============================================================================
# Mode: web  (local gateway + SvelteKit Vite dev server on :5173)
# =============================================================================

_start_web() {
	_start_infra_and_gateway

	# The web app's checked-in app-local env may point at other development
	# backends. In this mode the gateway is the one started above on :8080, so
	# export the public URL before Vite loads its env files.
	: "${PUBLIC_API_URL:=http://localhost:8080}"
	export PUBLIC_API_URL

	local web_port_pids
	web_port_pids="$(lsof -ti tcp:5173 2>/dev/null || true)"
	if [[ -n "$web_port_pids" ]]; then
		echo "⏹   Stopping existing listener on :5173…"
		kill $web_port_pids 2>/dev/null || true
	fi

	echo "▶  Starting SvelteKit web dev server on :5173…"
	pnpm --filter web dev --port 5173 --strictPort &
	PIDS+=($!)

	_wait_http "http://localhost:5173" '^(200|30[0-9])$' 45
	echo "✅  Web dev ready"
	echo ""
	echo "  Gateway:  http://localhost:8080"
	echo "  Web dev:  http://localhost:5173"
	echo ""
	echo "  Press Ctrl-C to stop all services."
	echo ""
	wait "${PIDS[@]}"
}

# =============================================================================
# Mode: tauri <platform>
# =============================================================================

_start_tauri() {
	local platform="${1:-macos}"

	# Validate platform before starting infra
	case "$platform" in
		macos|linux|desktop) ;;
		ios)
			if ! command -v xcodebuild &>/dev/null; then
				echo "❌  Xcode not found. Install Xcode to build for iOS."
				exit 1
			fi
			;;
		android)
			if [[ -z "${ANDROID_HOME:-}" ]]; then
				echo "❌  ANDROID_HOME is not set. Install Android Studio and set ANDROID_HOME."
				exit 1
			fi
			;;
		windows)
			echo "❌  Tauri Windows builds must be run on a Windows machine."
			echo "    Cross-compilation from macOS/Linux is not supported by Tauri."
			exit 1
			;;
		*)
			echo "❌  Unknown Tauri platform: '$platform'"
			echo "    Valid targets: macos  ios  android  linux  desktop"
			exit 1
			;;
	esac

	_start_infra_and_gateway

	# iOS physical-device support: the device must reach the Vite dev server
	# running on the host. TAURI_DEV_HOST is the host's LAN IP.
	# For the simulator, localhost works and TAURI_DEV_HOST is not needed.
	if [[ "$platform" == "ios" && -z "${TAURI_DEV_HOST:-}" ]]; then
		# Try to detect the active LAN interface IP (macOS: en0 / en1)
		local host_ip=""
		host_ip="$(ipconfig getifaddr en0 2>/dev/null \
			|| ipconfig getifaddr en1 2>/dev/null \
			|| hostname -I 2>/dev/null | awk '{print $1}' \
			|| true)"
		if [[ -n "$host_ip" ]]; then
			export TAURI_DEV_HOST="$host_ip"
			echo "ℹ️   TAURI_DEV_HOST=$TAURI_DEV_HOST  (override with TAURI_DEV_HOST=<ip>)"
		else
			echo "⚠️   Could not detect LAN IP. Physical device builds may fail."
			echo "    Set TAURI_DEV_HOST=<your-lan-ip> in .env.local to fix this."
		fi
	fi

	echo ""
	echo "▶  Launching Tauri ($platform)…"
	echo "   Vite dev server will start automatically on :5174"
	echo ""

	# Run Tauri in the foreground from the browser-shell directory.
	# When the user closes the app / presses Ctrl-C, _cleanup kills the gateway.
	cd "$BROWSER_SHELL_DIR"
	case "$platform" in
		macos|linux|desktop)
			pnpm tauri dev
			;;
		ios)
			pnpm tauri ios dev
			;;
		android)
			pnpm tauri android dev
			;;
	esac
}

# =============================================================================
# Mode: stop  (delegates to stop.sh)
# =============================================================================

_stop() {
	exec "$ROOT_DIR/stop.sh" "$@"
}

# =============================================================================
# Help
# =============================================================================

_usage() {
	cat <<'EOF'
Usage:
  ./start.sh [mode] [options]

──────────────────────────────────────────────────────────────────
DOCKER MODES  (gateway runs as a Docker container)
──────────────────────────────────────────────────────────────────
  (none) / full        Full stack: gateway + web + infra (default)
  infra                Qdrant + RustFS only
  observability        Jaeger + OTel collector

──────────────────────────────────────────────────────────────────
DEV MODES  (gateway runs as a local binary — faster iteration)
──────────────────────────────────────────────────────────────────
  local                Docker infra + compiled gateway on :8080
  web                  local + SvelteKit dev server on :5173
  tauri                local + Tauri desktop (macOS / Linux)
  tauri ios            local + Tauri iOS Simulator   [Xcode required]
  tauri android        local + Tauri Android emulator [Android Studio required]

──────────────────────────────────────────────────────────────────
OTHER
──────────────────────────────────────────────────────────────────
  stop [profile] [wipe]   Stop Docker profile (full|infra|observability)
  -h, --help              This help

──────────────────────────────────────────────────────────────────
ENVIRONMENT (.env.local is always loaded first)
──────────────────────────────────────────────────────────────────
  ANTHROPIC_API_KEY          Required for LLM calls
  CONUSAI_BUILD_GATEWAY=1    Rebuild gateway binary before dev modes start
  TAURI_DEV_HOST=<ip>        Override LAN IP for Tauri iOS physical device
                             (auto-detected; only needed for real hardware)

──────────────────────────────────────────────────────────────────
FIRST-TIME SETUP
──────────────────────────────────────────────────────────────────
  cp .env.example .env.local            # add your ANTHROPIC_API_KEY
  cargo build --bin agent-gateway       # compile the gateway
  ./start.sh local                      # run the platform

  # Or with local embedding model (no OpenAI key needed for embeddings):
  cargo build --bin agent-gateway --features agent-gateway/local-embeddings
  ./start.sh web                        # gateway + web dev server
  ./start.sh tauri                      # gateway + Tauri macOS app
  ./start.sh tauri ios                  # gateway + Tauri iOS Simulator
EOF
}

# =============================================================================
# Main
# =============================================================================

MODE="${1:-full}"
SUBMODE="${2:-}"

# Optional: rebuild gateway binary before any dev mode.
if [[ "${CONUSAI_BUILD_GATEWAY:-0}" == "1" ]] && [[ "$MODE" =~ ^(local|web|tauri)$ ]]; then
	echo "▶  Building gateway binary (CONUSAI_BUILD_GATEWAY=1)…"
	cargo build --bin agent-gateway --features agent-gateway/local-embeddings
	echo "✅  Build complete"
fi

case "$MODE" in
	full|infra|observability)
		_start_docker_profile "$MODE"
		;;
	local)
		_start_local
		;;
	web)
		_start_web
		;;
	tauri)
		_start_tauri "$SUBMODE"
		;;
	stop)
		shift
		_stop "$@"
		;;
	-h|--help|help)
		_usage
		;;
	*)
		echo "❌  Unknown mode: '$MODE'"
		echo ""
		_usage
		exit 1
		;;
esac
