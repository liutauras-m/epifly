#!/usr/bin/env bash
# Pre-create the Docker named volumes used by the Conusai stacks.
# Run ONCE on the Dokploy host before the first `conusai-infra` deploy.
#
# Volumes are declared `external: true` in the compose files so their
# lifecycle is decoupled from the stack — they survive project deletes,
# `docker compose down -v`, and Dokploy redeploys.
#
# Usage (on the Dokploy host, via Dokploy → Server → Terminal or SSH):
#   curl -fsSL https://raw.githubusercontent.com/liutauras-m/epifly/main/dokploy/bootstrap-volumes.sh | bash
# or:
#   bash dokploy/bootstrap-volumes.sh

set -euo pipefail

VOLUMES=(
  conusai_postgres_data
  conusai_redis_data
  conusai_qdrant_data
  conusai_rustfs_data
  conusai_redb_data
)

for v in "${VOLUMES[@]}"; do
  if docker volume inspect "$v" >/dev/null 2>&1; then
    echo "• $v already exists"
  else
    docker volume create "$v" >/dev/null
    echo "✔ created $v"
  fi
done

echo
echo "Done. Verify with: docker volume ls | grep conusai_"
