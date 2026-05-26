default:
    @just --list

# ── Frontend ──────────────────────────────────────────────────────────────────
#
# Best-practice topology (see docs/ui-plan.md §"Quick-start order"):
#   • `./start.sh local`     — backend + infra (Docker), gateway on host
#   • `just web-dev`         — SvelteKit web app on :5173 (host process)
#   • `just shell-dev`       — Tauri desktop shell, Vite on :5174 (host process)
#   • `just shell-ios-dev`   — Tauri iOS simulator       (VITE_API_BASE=127.0.0.1)
#   • `just shell-android-dev` — Tauri Android emulator  (VITE_API_BASE=10.0.2.2)
#
# Each command is long-running; launch in its own terminal. Frontends never run
# in Docker — Vite/Tauri HMR and native sims require host execution.

# Backend URL each surface should talk to. Override per-shell when needed:
#   just shell-android-dev BACKEND=http://10.0.2.2:8080
BACKEND := env_var_or_default("CONUSAI_BACKEND_URL", "http://localhost:8080")

# Web (browser, host process — proxies to BACKEND via vite.config.ts).
web-dev:
    CONUSAI_BACKEND_URL={{BACKEND}} pnpm --filter web dev

web-build:
    CONUSAI_BACKEND_URL={{BACKEND}} pnpm --filter web build

# Desktop Tauri shell. localhost:8080 reaches the host gateway from the webview.
shell-dev:
    VITE_API_BASE={{BACKEND}} pnpm --filter browser-shell tauri dev

shell-build:
    VITE_API_BASE={{BACKEND}} pnpm --filter browser-shell tauri build

shell-build-macos:
    VITE_API_BASE={{BACKEND}} pnpm --filter browser-shell tauri build -- --target universal-apple-darwin

shell-build-windows:
    VITE_API_BASE={{BACKEND}} pnpm --filter browser-shell tauri build -- --target x86_64-pc-windows-msvc

# iOS simulator: webview can reach the host via 127.0.0.1.
shell-ios-dev:
    VITE_API_BASE=http://127.0.0.1:8080 pnpm --filter browser-shell tauri ios dev

shell-ios-build:
    VITE_API_BASE={{BACKEND}} pnpm --filter browser-shell tauri ios build --target aarch64

# Android emulator: 10.0.2.2 is the special host-loopback alias.
shell-android-dev:
    VITE_API_BASE=http://10.0.2.2:8080 pnpm --filter browser-shell tauri android dev

shell-android-build:
    VITE_API_BASE={{BACKEND}} pnpm --filter browser-shell tauri android build --apk

shell-android-aab:
    VITE_API_BASE={{BACKEND}} pnpm --filter browser-shell tauri android build --aab

# Umbrella: bring up backend infra + web dev server in two terminals (tmux).
# Falls back to instructions if tmux is not installed.
dev:
    @if command -v tmux >/dev/null 2>&1; then \
        tmux new-session -d -s conusai './start.sh local' \; \
            split-window -h 'just web-dev' \; \
            attach; \
    else \
        echo "Install tmux for one-command dev, or run in two terminals:"; \
        echo "  1) ./start.sh local"; \
        echo "  2) just web-dev"; \
    fi

# Stop host-side frontend dev servers (does not touch Docker infra).
stop-frontend:
    -pkill -f "vite.*--port 5173" 2>/dev/null
    -pkill -f "vite.*--port 5174" 2>/dev/null
    -pkill -f "tauri (dev|ios|android)" 2>/dev/null
    @echo "✅ Host frontend processes stopped"

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

# ── Deploy / Eval ─────────────────────────────────────────────────────────────
# Requires: DOKPLOY_URL, DOKPLOY_API_KEY, DOKPLOY_ENVIRONMENT_ID, APP_DOMAIN
# set in the shell (or present in dokploy/.dokploy).
#
# Usage:
#   just epifly-build           build the CLI (auto-run by deploy/eval targets)
#   just deploy-beta            full deploy (6-phase orchestrator + Phase 5 verify)
#   just eval-beta              post-deploy eval: HTTP smoke + service diagnostics
#   just deploy-eval-beta       deploy then eval in one shot

# Build the epifly CLI (idempotent — tsup only rebuilds on change).
epifly-build:
    pnpm --filter @conusai/epifly build

# Deploy to beta environment via epifly CLI.
# Triggers the epifly-deploy Dokploy compose, streams logs, runs Phase 5 verify.
deploy-beta: epifly-build
    node tools/epifly/dist/epifly.mjs deploy --config dokploy/.dokploy --timeout 900

# Post-deploy evaluation: HTTP smoke tests + Dokploy service diagnostics.
eval-beta: epifly-build
    node tools/epifly/dist/epifly.mjs verify --config dokploy/.dokploy
    node tools/epifly/dist/epifly.mjs doctor --config dokploy/.dokploy

# Full deploy + eval in one command.
deploy-eval-beta: deploy-beta eval-beta

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

# ── UI design-system gates (Phase 1) ─────────────────────────────────────────

# Generate docs/ui-inventory.md (component list, route list, style violations).
ui-inventory:
    node scripts/dump-ui-inventory.mjs

# Token audit — fails CI on raw hex / px / cubic-bezier outside token files.
# Pass --warn to report without failing (useful for first-run baseline).
ui-tokens *args:
    node scripts/check-design-tokens.mjs {{args}}

# Visual regression — runs Playwright inside the official Docker image so font
# rendering is byte-identical across macOS / Linux / Windows / CI.
# First run: add --update-snapshots to commit the baseline.
visual *args:
    docker run --rm --network host \
      -v $PWD:/work -w /work \
      mcr.microsoft.com/playwright:v1.49.0-jammy \
      pnpm --filter web exec playwright test --project=visual {{args}}

# Accessibility + performance audit (requires the web dev server to be running).
# Snapshot scores to test-results/audit-phase-N/; gate further phases on no regression.
ui-audit:
    pnpm --filter web exec playwright test --project=chromium e2e/a11y.spec.ts

verify:
    cargo clippy --workspace -- -D warnings
    just lint-tenant-paths
    just lint-capability-storage
    just ui-tokens --warn
    pnpm -w lint:svelte
    pnpm -w lint
    pnpm -w test
    cargo test --workspace
    just types
    git diff --exit-code packages/types/src

# Consolidated audit gate used for cleanup/refactor cycles.
gate:
    pnpm install --frozen-lockfile
    pnpm -w check
    pnpm -w lint
    pnpm -w test
    pnpm -w build
    pnpm -w knip
    cargo machete --with-metadata
    cargo run -p xtask -- validate-capabilities --strict
    cargo fmt --all --check
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    cargo test --workspace --all-features
    cargo audit
    cargo deny check
    make verify-routes-doc
