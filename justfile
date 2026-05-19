default:
    @just --list

# ── Frontend ──────────────────────────────────────────────────────────────────

web-dev:
    pnpm --filter web dev

shell-dev:
    pnpm --filter browser-shell tauri dev

shell-build:
    pnpm --filter browser-shell tauri build

shell-build-macos:
    pnpm --filter browser-shell tauri build -- --target universal-apple-darwin

shell-build-windows:
    pnpm --filter browser-shell tauri build -- --target x86_64-pc-windows-msvc

shell-ios-dev:
    pnpm --filter browser-shell tauri ios dev

shell-ios-build:
    pnpm --filter browser-shell tauri ios build --target aarch64

shell-android-dev:
    pnpm --filter browser-shell tauri android dev

shell-android-build:
    pnpm --filter browser-shell tauri android build --apk

shell-android-aab:
    pnpm --filter browser-shell tauri android build --aab

# ── Backend ───────────────────────────────────────────────────────────────────

backend-dev:
    cargo run -p agent-gateway

backend-check:
    cargo check --workspace

backend-test:
    cargo test --workspace

# ── Codegen ───────────────────────────────────────────────────────────────────

types:
    ./scripts/openapi-to-types.sh

# ── Full verify (CI gate) ─────────────────────────────────────────────────────

# ── Tenant path lint (CI guard) ───────────────────────────────────────────────
# Rejects any hand-rolled `tenants/{...}` or starts_with/strip_prefix("tenants/")
# literals outside the single source-of-truth module (tenant_storage.rs).
lint-tenant-paths:
    @! grep -rnE 'tenants/\{|format!\("tenants/|starts_with\("tenants/|strip_prefix\("tenants/' \
        --include='*.rs' \
        --exclude-dir=target \
        apps/backend/crates \
      | grep -vE ':[[:space:]]*//' \
      | grep -v 'apps/backend/crates/agent-core/src/store/tenant_storage.rs' \
      | grep -v 'apps/backend/crates/rustfs-admin/' \
      | grep -v 'apps/backend/crates/common/src/path_safety.rs' \
      | grep -v 'apps/backend/crates/jobs/src/jobs/tenant_bucket_migration.rs' \
      | grep -v 'apps/backend/crates/agent-gateway/src/routes/mod.rs' \
      || (echo "ERROR: Forbidden hand-rolled tenant path literal outside tenant_storage.rs"; exit 1)

# ── Capability storage isolation guard (CI guard) ─────────────────────────────
# Rejects capability files that import object_store, TenantStorage, or S3_BUCKET
# directly — capabilities must use Arc<dyn WorkspaceStorage> only.
lint-capability-storage:
    @! grep -rnE 'use object_store|TenantStorage|S3_BUCKET|format!\("tenants/' \
        --include='*.rs' \
        --exclude-dir=target \
        apps/backend/crates/agent-gateway/src/capabilities \
      || (echo "ERROR: Capability code must not access object_store/TenantStorage/S3_BUCKET directly — use Arc<dyn WorkspaceStorage>"; exit 1)

# ── Tenant bucket migration (Phase 2 operator gate) ──────────────────────────
# Migrates tenants from the shared `workspace` bucket to per-tenant `ws-{id}` buckets.
# Safe to re-run; already-migrated tenants are skipped.
#
#   just migrate-tenant-buckets              # migrate all pending tenants
#   just migrate-tenant-buckets -- --dry-run # print plan, no data movement
#   just migrate-tenant-buckets -- --tenant acme-corp  # canary single tenant
migrate-tenant-buckets *args:
    @echo "Triggering tenant bucket migration job..."
    @MIGRATION_DRY_RUN=$(echo "{{args}}" | grep -q '\-\-dry-run' && echo true || echo false) \
     MIGRATION_TENANT_ID=$(echo "{{args}}" | grep -oP '(?<=--tenant )\S+' || true) \
     cargo run -p agent-gateway --bin trigger-job -- tenant-bucket-migration

verify:
    cargo clippy --workspace -- -D warnings
    just lint-tenant-paths
    just lint-capability-storage
    pnpm -w lint
    pnpm -w test
    cargo test --workspace
    just types
    git diff --exit-code packages/types/src
