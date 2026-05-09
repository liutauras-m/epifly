#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  ./stop.sh [profile] [wipe]
  ./stop.sh [wipe] [profile]

Profiles:
  infra          Postgres + MinIO (+ minio-init)
  full           Full stack (gateway + infra + observability)
  observability  Jaeger + OTel collector

Options:
  wipe, --wipe, -w, ti
                 Stop and remove project containers, volumes, networks,
                 and project images to start from a clean slate.
EOF
}

PROFILE="infra"
WIPE=false

for arg in "$@"; do
  case "$arg" in
    infra|full|observability)
      PROFILE="$arg"
      ;;
    wipe|--wipe|-w|ti)
      WIPE=true
      ;;
    -h|--help|help)
      usage
      exit 0
      ;;
    *)
      echo "Invalid argument: $arg"
      usage
      exit 1
      ;;
  esac
done

case "$PROFILE" in
  infra|full|observability)
    ;;
  *)
    echo "Invalid profile: $PROFILE"
    usage
    exit 1
    ;;
esac

if [ "$WIPE" = true ]; then
  echo "Stopping and wiping ConusAI Platform profile: $PROFILE"
  docker compose --profile "$PROFILE" down --volumes --rmi all --remove-orphans
  echo "Wipe complete"
else
  echo "Stopping ConusAI Platform profile: $PROFILE"
  docker compose --profile "$PROFILE" down
  echo "Profile stopped"
fi
