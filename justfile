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

verify:
    cargo clippy --workspace -- -D warnings
    pnpm -w lint
    pnpm -w test
    cargo test --workspace
    just types
    git diff --exit-code packages/types/src
