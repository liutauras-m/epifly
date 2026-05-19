#!/usr/bin/env bash
set -euo pipefail

# Root entrypoint for starting/stopping the platform.
# Default behavior starts the full profile so all services come up.
ROOT_DIR="$(cd "$(dirname "$0")" && pwd)"
BACKEND_START="$ROOT_DIR/apps/backend/start.sh"
LOCAL_GATEWAY_BIN="$ROOT_DIR/target/debug/agent-gateway"

usage() {
	cat <<'EOF'
Usage:
  ./start.sh [full|infra|observability]
  ./start.sh local
  ./start.sh stop [full|infra|observability]

Modes:
  full           Start the full Docker stack (default)
  infra          Start infra-only Docker profile
  observability  Start observability profile
  local          Start full stack, stop Docker gateway, run local gateway binary
  stop           Stop a Docker profile (default profile: full)
EOF
}

load_env_files() {
	if [ -f "$ROOT_DIR/.env.local" ]; then
		set -a
		# shellcheck disable=SC1091
		source "$ROOT_DIR/.env.local"
		set +a
	fi
}

ensure_lago_rsa_key() {
	local env_file="$ROOT_DIR/.env.local"

	# If already set in the environment (sourced from .env.local), use it.
	if [ -n "${LAGO_RSA_PRIVATE_KEY:-}" ]; then
		return 0
	fi

	# Lago expects a base64-encoded RSA private key; generate a local dev fallback
	# and persist it into .env.local so restarts reuse the same key.
	LAGO_RSA_PRIVATE_KEY="$(openssl genrsa 2048 2>/dev/null | base64 | tr -d '\n')"
	export LAGO_RSA_PRIVATE_KEY
	echo "LAGO_RSA_PRIVATE_KEY=$LAGO_RSA_PRIVATE_KEY" >> "$env_file"
	echo "ℹ️  Generated LAGO_RSA_PRIVATE_KEY and saved to .env.local"
}

wait_http() {
	local url="$1"
	local expected_regex="$2"
	local timeout_secs="${3:-90}"
	local start_ts
	start_ts="$(date +%s)"

	while true; do
		local code
		code="$(curl -s -o /dev/null -w "%{http_code}" "$url" || true)"
		if [[ "$code" =~ $expected_regex ]]; then
			return 0
		fi
		if (( "$(date +%s)" - start_ts >= timeout_secs )); then
			echo "❌ Timeout waiting for $url (last code: $code)"
			return 1
		fi
		sleep 2
	done
}

report_unhealthy() {
	local unhealthy
	unhealthy="$(docker ps --filter health=unhealthy --format '{{.Names}}' | grep '^conusai-' || true)"
	if [ -n "$unhealthy" ]; then
		echo "❌ Unhealthy ConusAI containers detected:"
		echo "$unhealthy"
		return 1
	fi
	return 0
}

start_profile() {
	local profile="$1"
	ensure_lago_rsa_key
	echo "▶ Starting ConusAI profile: $profile"
	if ! "$BACKEND_START" "$profile"; then
		echo "⚠️ Initial startup failed; retrying profile once..."
		docker compose --profile "$profile" up -d
		docker compose up -d --force-recreate lago-api lago-worker >/dev/null 2>&1 || true
		docker compose restart zitadel >/dev/null 2>&1 || true
	fi

	if [ "$profile" = "full" ]; then
		wait_http "http://localhost:8080/health" '^200$' 120
		wait_http "http://localhost:3000/login" '^(200|30[1278])$' 120
		wait_http "http://localhost:9001/login" '^(200|403)$' 120
		report_unhealthy
	fi

	echo "✅ Profile is up"
}

start_local_gateway() {
	if [ ! -x "$LOCAL_GATEWAY_BIN" ]; then
		echo "❌ Local gateway binary not found: $LOCAL_GATEWAY_BIN"
		echo "   Build it first: cargo build --bin agent-gateway"
		exit 1
	fi

	# Kill any stale host gateway before touching files or starting Docker.
	pkill -f "target/debug/agent-gateway" 2>/dev/null || true
	sleep 0.5

	load_env_files

	: "${REDB_PATH:=/tmp/conusai-local-gw.redb}"
	# DO NOT delete the redb file — it persists data across restarts.
	# Data loss should only happen on explicit `stop wipe`, not on every startup.

	ensure_lago_rsa_key

	# Bring up all services EXCEPT the Docker gateway container.
	# List services explicitly — --scale 0 still builds the image; this avoids it.
	# current-time is excluded because it depends_on: agent-gateway (triggers build).
	echo "▶ Starting infrastructure (full profile, no Docker gateway)..."
	docker compose up -d --no-build postgres redis zitadel lago-api lago-worker qdrant rustfs-perms rustfs web

	# Wait for the services the gateway itself depends on.
	wait_http "http://localhost:6333/healthz" '^200$' 60
	echo "✅ Qdrant ready"
	wait_http "http://localhost:9000" '^(200|403)$' 60
	echo "✅ RustFS ready"
	wait_http "http://localhost:3000/login" '^(200|30[1278])$' 120
	echo "✅ Web ready"
	report_unhealthy

	: "${QDRANT_URL:=http://localhost:6334}"
	: "${S3_ENDPOINT:=http://localhost:9000}"
	: "${S3_BUCKET:=workspace}"
	: "${RUSTFS_ROOT_ACCESS_KEY:=rustfsadmin}"
	: "${RUSTFS_ROOT_SECRET_KEY:=rustfsadmin}"
	: "${RUSTFS_BOOTSTRAP:=on}"
	: "${RUSTFS_VERSIONING:=on}"
	: "${RUSTFS_PER_TENANT_IAM:=on}"
	: "${RUSTFS_REAL_PRESIGN:=on}"
	: "${RUSTFS_SSE:=on}"
	: "${RUSTFS_NOTIFICATIONS:=on}"
	: "${RUSTFS_QUOTAS:=on}"
	: "${RUSTFS_NOTIFICATION_WEBHOOK_URL:=http://localhost:8080/internal/rustfs/events}"
	: "${UI_SESSION_KEY:=conusai-foundry-dev-secret-change-me-32b}"
	: "${WEB_ORIGIN:=http://localhost:3000,http://localhost:5173}"
	: "${EMBEDDING_BACKEND:=local}"
	: "${CONUSAI_CAPABILITIES_DIR:=./capabilities}"
	: "${RUST_LOG:=info}"

	export QDRANT_URL S3_ENDPOINT S3_BUCKET REDB_PATH
	export RUSTFS_ROOT_ACCESS_KEY RUSTFS_ROOT_SECRET_KEY RUSTFS_BOOTSTRAP
	export RUSTFS_VERSIONING RUSTFS_PER_TENANT_IAM RUSTFS_REAL_PRESIGN
	export RUSTFS_SSE RUSTFS_NOTIFICATIONS RUSTFS_QUOTAS
	export RUSTFS_NOTIFICATION_WEBHOOK_URL UI_SESSION_KEY WEB_ORIGIN
	export EMBEDDING_BACKEND CONUSAI_CAPABILITIES_DIR RUST_LOG

	echo "▶ Starting local gateway binary on :8080"
	cd "$ROOT_DIR/apps/backend"
	exec "$LOCAL_GATEWAY_BIN"
}

if [ $# -eq 0 ]; then
	start_profile full
	exit 0
fi

case "${1:-}" in
	local)
		start_local_gateway
		;;
	full|infra|observability)
		start_profile "$1"
		;;
	stop)
		profile="${2:-full}"
		exec "$BACKEND_START" stop "$profile"
		;;
	-h|--help|help)
		usage
		;;
	*)
		echo "❌ Unsupported mode: $1"
		usage
		exit 1
		;;
esac
