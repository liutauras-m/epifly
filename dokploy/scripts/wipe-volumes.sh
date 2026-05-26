#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# wipe-volumes.sh — destructively wipe Epifly stateful Docker volumes
# ─────────────────────────────────────────────────────────────────────────────
# Use this when:
#   - You changed a stateful secret (ZITADEL_MASTERKEY, LAGO_ENCRYPTION_*,
#     LAGO_RSA_PRIVATE_KEY, RUSTFS_IAM_ENC_KEY) and the previous data on disk
#     is no longer decryptable.
#   - You renamed a Lago env var (e.g. ENCRYPTION_PRIMARY_KEY →
#     LAGO_ENCRYPTION_PRIMARY_KEY) and need to reinitialise the lago DB
#     with the correct keys.
#   - You want to start the project from a clean slate.
#
# RUN THIS ON THE DOCKER HOST (not your laptop), e.g.:
#   ssh root@beta.test.cloud.conusai.com
#   curl -fsSL https://raw.githubusercontent.com/<org>/<repo>/main/dokploy/scripts/wipe-volumes.sh -o /tmp/wipe.sh
#   bash /tmp/wipe.sh --lago --yes
#
# Or scp the script up and run it locally.
#
# Examples:
#   ./wipe-volumes.sh --lago --yes        # wipe lago DB + lago storage only
#   ./wipe-volumes.sh --postgres --yes    # wipe entire postgres volume
#                                         # (zitadel + lago + future apps)
#   ./wipe-volumes.sh --all --yes         # wipe everything
#   ./wipe-volumes.sh --lago              # dry-run — show what would happen
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

# Volumes managed by dokploy/infra/docker-compose.yml. Keep in sync with
# EXTERNAL_VOLUMES in dokploy/epifly-deploy/scripts/deploy.mjs.
POSTGRES_VOL="conusai_postgres_data"
REDIS_VOL="conusai_redis_data"
QDRANT_VOL="conusai_qdrant_data"
RUSTFS_VOL="conusai_rustfs_data"
REDB_VOL="conusai_redb_data"
LAGO_STORAGE_VOL="conusai_lago_storage_data"

ALL_VOLS=(
  "$POSTGRES_VOL" "$REDIS_VOL" "$QDRANT_VOL"
  "$RUSTFS_VOL" "$REDB_VOL" "$LAGO_STORAGE_VOL"
)

# Containers that may have a volume mounted at run time. `docker volume rm`
# refuses if a container references the volume, so we stop them first.
# Service-name based — works with any compose project prefix.
COMPOSE_SERVICES=(
  postgres pg-password-sync redis qdrant rustfs rustfs-perms
  zitadel lago-api lago-worker lago-clock lago-migrate
)

# Naming policy: backup files go to /var/backups/epifly so they're outside
# the repo and survive a `docker volume rm`.
BACKUP_DIR="${BACKUP_DIR:-/var/backups/epifly}"
TS="$(date -u +%Y%m%dT%H%M%SZ)"

# ── arg parsing ─────────────────────────────────────────────────────────────
WANT_LAGO=0
WANT_POSTGRES=0
WANT_REDIS=0
WANT_QDRANT=0
WANT_RUSTFS=0
WANT_REDB=0
WANT_LAGO_STORAGE=0
WANT_ALL=0
NO_BACKUP=0
YES=0

usage() {
  cat <<'EOF'
Usage: wipe-volumes.sh [TARGETS] [--yes] [--no-backup]

Targets (pick one or more):
  --lago              Drop the `lago` database inside conusai_postgres_data
                      AND wipe conusai_lago_storage_data. Keeps zitadel data.
  --postgres          Wipe conusai_postgres_data (zitadel + lago + all apps).
  --redis             Wipe conusai_redis_data.
  --qdrant            Wipe conusai_qdrant_data.
  --rustfs            Wipe conusai_rustfs_data (S3 object storage).
  --redb              Wipe conusai_redb_data.
  --lago-storage      Wipe conusai_lago_storage_data only.
  --all               Wipe every volume listed above.

Flags:
  --yes               Skip the confirmation prompt. Required for non-interactive use.
  --no-backup         Skip pg_dump backup before --lago / --postgres wipe.
  -h, --help          Show this help.

Without --yes the script runs in DRY-RUN mode (prints what it would do).

Examples:
  wipe-volumes.sh --lago --yes
  wipe-volumes.sh --postgres --yes
  wipe-volumes.sh --all --yes --no-backup
EOF
}

if [[ $# -eq 0 ]]; then
  usage
  exit 0
fi

while [[ $# -gt 0 ]]; do
  case "$1" in
    --lago)         WANT_LAGO=1 ;;
    --postgres)     WANT_POSTGRES=1 ;;
    --redis)        WANT_REDIS=1 ;;
    --qdrant)       WANT_QDRANT=1 ;;
    --rustfs)       WANT_RUSTFS=1 ;;
    --redb)         WANT_REDB=1 ;;
    --lago-storage) WANT_LAGO_STORAGE=1 ;;
    --all)          WANT_ALL=1 ;;
    --yes|-y)       YES=1 ;;
    --no-backup)    NO_BACKUP=1 ;;
    -h|--help)      usage; exit 0 ;;
    *)              echo "✗ Unknown arg: $1" >&2; usage; exit 2 ;;
  esac
  shift
done

if [[ $WANT_ALL -eq 1 ]]; then
  WANT_POSTGRES=1; WANT_REDIS=1; WANT_QDRANT=1
  WANT_RUSTFS=1; WANT_REDB=1; WANT_LAGO_STORAGE=1
fi

# --lago is shorthand: lago DB inside postgres + lago storage
# We do NOT set WANT_POSTGRES here — we drop just the `lago` DB below.
if [[ $WANT_LAGO -eq 1 ]]; then
  WANT_LAGO_STORAGE=1
fi

# ── helpers ─────────────────────────────────────────────────────────────────
log()  { printf '\033[36m·\033[0m %s\n' "$*"; }
done_(){ printf '\033[32m✓\033[0m %s\n' "$*"; }
warn() { printf '\033[33m⚠\033[0m %s\n' "$*" >&2; }
fail() { printf '\033[31m✗\033[0m %s\n' "$*" >&2; exit 1; }

require_docker() {
  command -v docker >/dev/null 2>&1 || fail "docker CLI not found on PATH"
  docker info >/dev/null 2>&1 || fail "cannot reach the Docker daemon (need root or docker group)"
}

volume_exists() {
  docker volume inspect "$1" >/dev/null 2>&1
}

find_container_for_service() {
  # Find a container whose com.docker.compose.service label matches $1.
  # Works regardless of the compose project name Dokploy generates.
  docker ps -a --filter "label=com.docker.compose.service=$1" --format '{{.ID}}'
}

stop_consumers() {
  log "Stopping containers that may hold open volume handles…"
  local stopped=0
  for svc in "${COMPOSE_SERVICES[@]}"; do
    while IFS= read -r cid; do
      [[ -z "$cid" ]] && continue
      docker stop --time 30 "$cid" >/dev/null
      stopped=$((stopped + 1))
    done < <(find_container_for_service "$svc")
  done
  done_ "Stopped $stopped container(s)."
}

backup_postgres() {
  # pg_dump the whole cluster before destruction. Skips silently when the
  # postgres container isn't running (nothing to back up).
  [[ $NO_BACKUP -eq 1 ]] && { warn "Skipping pg_dump (--no-backup)"; return; }

  local cid
  cid="$(docker ps --filter "label=com.docker.compose.service=postgres" --format '{{.ID}}' | head -1)"
  if [[ -z "$cid" ]]; then
    warn "postgres container not running — skipping pg_dump (no backup taken)"
    return
  fi

  mkdir -p "$BACKUP_DIR"
  local out="$BACKUP_DIR/postgres-${TS}.sql.gz"
  log "Dumping postgres → $out"
  # `pg_dumpall` captures roles + databases. Local Unix socket → no password
  # needed (postgres image's pg_hba.conf trusts local connections).
  docker exec "$cid" \
    sh -c 'pg_dumpall -U "${POSTGRES_USER:-conusai}" -h /var/run/postgresql' \
    | gzip > "$out"
  done_ "pg_dump complete ($(du -h "$out" | cut -f1))"
}

drop_lago_db() {
  local cid
  cid="$(docker ps --filter "label=com.docker.compose.service=postgres" --format '{{.ID}}' | head -1)"
  if [[ -z "$cid" ]]; then
    # Postgres isn't running — start it briefly so we can drop the DB.
    warn "postgres not running; attempting to start the volume in a one-shot container"
    docker run --rm -i \
      -v "${POSTGRES_VOL}:/var/lib/postgresql/data" \
      -e POSTGRES_USER="${POSTGRES_USER:-conusai}" \
      -e POSTGRES_PASSWORD="${POSTGRES_PASSWORD:-changeme}" \
      --entrypoint sh \
      postgres:17-alpine -c '
        set -e
        # The default entrypoint refuses to start without PGDATA being
        # initialised; if this volume is empty, there is no lago DB to drop.
        if [ ! -s /var/lib/postgresql/data/PG_VERSION ]; then
          echo "✓ postgres volume is empty — nothing to drop"
          exit 0
        fi
        echo "✗ refusing to mutate postgres data without a live container — start the stack first" >&2
        exit 1
      '
    return
  fi
  log "Dropping & recreating the \`lago\` database (preserves zitadel)…"
  docker exec -i "$cid" psql -U "${POSTGRES_USER:-conusai}" -v ON_ERROR_STOP=1 -d postgres <<'SQL'
    SELECT pg_terminate_backend(pid)
      FROM pg_stat_activity
     WHERE datname = 'lago' AND pid <> pg_backend_pid();
    DROP DATABASE IF EXISTS lago;
    CREATE DATABASE lago;
SQL
  done_ "lago database dropped & recreated"
}

remove_volume() {
  local vol="$1"
  if ! volume_exists "$vol"; then
    log "$vol — does not exist (skip)"
    return
  fi
  if [[ $YES -eq 0 ]]; then
    log "(dry-run) docker volume rm $vol"
    return
  fi
  docker volume rm "$vol" >/dev/null
  done_ "removed $vol"
}

# ── plan + confirm ──────────────────────────────────────────────────────────
require_docker

PLAN=()
[[ $WANT_LAGO          -eq 1 ]] && PLAN+=("DROP DATABASE lago inside $POSTGRES_VOL (zitadel data preserved)")
[[ $WANT_POSTGRES      -eq 1 ]] && PLAN+=("REMOVE VOLUME $POSTGRES_VOL (zitadel + lago + ALL app data)")
[[ $WANT_REDIS         -eq 1 ]] && PLAN+=("REMOVE VOLUME $REDIS_VOL")
[[ $WANT_QDRANT        -eq 1 ]] && PLAN+=("REMOVE VOLUME $QDRANT_VOL")
[[ $WANT_RUSTFS        -eq 1 ]] && PLAN+=("REMOVE VOLUME $RUSTFS_VOL (all S3 objects)")
[[ $WANT_REDB          -eq 1 ]] && PLAN+=("REMOVE VOLUME $REDB_VOL")
[[ $WANT_LAGO_STORAGE  -eq 1 ]] && PLAN+=("REMOVE VOLUME $LAGO_STORAGE_VOL")

if [[ ${#PLAN[@]} -eq 0 ]]; then
  fail "No targets selected. See --help."
fi

echo "─────────────────────────────────────────────────────────────────"
echo "  wipe-volumes.sh — DESTRUCTIVE plan"
echo "─────────────────────────────────────────────────────────────────"
for line in "${PLAN[@]}"; do echo "  • $line"; done
echo "  Backup dir:  $BACKUP_DIR"
echo "  Backup:      $([[ $NO_BACKUP -eq 1 ]] && echo SKIPPED || echo enabled)"
echo "  Mode:        $([[ $YES -eq 1 ]] && echo APPLY || echo 'DRY RUN (add --yes to apply)')"
echo "─────────────────────────────────────────────────────────────────"
echo

if [[ $YES -eq 0 ]]; then
  warn "Dry-run — no changes made. Re-run with --yes to apply."
  exit 0
fi

# Interactive sanity check even when --yes is passed, IF a TTY is attached.
if [[ -t 0 && -t 1 ]]; then
  read -r -p "Type 'WIPE' to confirm: " confirm
  [[ "$confirm" == "WIPE" ]] || fail "Aborted (confirmation mismatch)."
fi

# ── execute ─────────────────────────────────────────────────────────────────
if [[ $WANT_LAGO -eq 1 || $WANT_POSTGRES -eq 1 ]]; then
  backup_postgres
fi

# Drop lago DB BEFORE stopping postgres — needs a live container.
if [[ $WANT_LAGO -eq 1 && $WANT_POSTGRES -eq 0 ]]; then
  drop_lago_db
fi

# Now stop everything so the volume handles release.
stop_consumers

[[ $WANT_POSTGRES     -eq 1 ]] && remove_volume "$POSTGRES_VOL"
[[ $WANT_REDIS        -eq 1 ]] && remove_volume "$REDIS_VOL"
[[ $WANT_QDRANT       -eq 1 ]] && remove_volume "$QDRANT_VOL"
[[ $WANT_RUSTFS       -eq 1 ]] && remove_volume "$RUSTFS_VOL"
[[ $WANT_REDB         -eq 1 ]] && remove_volume "$REDB_VOL"
[[ $WANT_LAGO_STORAGE -eq 1 ]] && remove_volume "$LAGO_STORAGE_VOL"

echo
done_ "All requested wipes complete."
echo
echo "Next steps:"
echo "  1. In Dokploy UI, redeploy the \`epifly-deploy\` compose"
echo "     (or push a new tag if it's tag-triggered)."
echo "  2. Phase 0 recreates the volumes empty."
echo "  3. Phase 1 regenerates any stateful secrets whose bound volume is gone."
echo "  4. lago-migrate runs db:setup on the fresh \`lago\` DB."
