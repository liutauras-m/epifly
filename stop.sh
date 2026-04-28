#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  ./stop.sh [profile]

Profiles:
  infra          Qdrant + MinIO (+ minio-init)
  full           Full stack (gateway + infra + observability)
  observability  Jaeger + OTel collector
EOF
}

PROFILE="infra"

if [ -n "${1:-}" ]; then
  PROFILE="$1"
fi

case "$PROFILE" in
  infra|full|observability)
    ;;
  -h|--help|help)
    usage
    exit 0
    ;;
  *)
    echo "Invalid profile: $PROFILE"
    usage
    exit 1
    ;;
esac

echo "Stopping ConusAI Platform profile: $PROFILE"
docker compose --profile "$PROFILE" down
echo "Profile stopped"
