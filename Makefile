# Makefile — convenience wrappers for the conusai-platform monorepo

.PHONY: verify fmt lint test build clean \
        test-e2e test-e2e-web test-e2e-ios test-e2e-shell start-tauri-driver

## Run full verification (fmt, clippy, tests)
verify:
	cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace

## Format all Rust code
fmt:
	cargo fmt --all

## Run clippy
lint:
	cargo clippy --workspace --all-targets -- -D warnings

## Run tests
test:
	cargo test --workspace

## Production build
build:
	cargo build --release --bin agent-gateway

## Clean build artefacts
clean:
	rm -rf apps/backend/target

## Start Postgres (TimescaleDB + pgvector)
db-up:
	docker compose --profile infra up -d postgres

## Tear down and recreate Postgres with a fresh volume
db-reset:
	docker compose rm -sf postgres
	docker volume rm conusai-platform_postgres_data || true
	$(MAKE) db-up

## Run sqlx migrations
db-migrate:
	DATABASE_URL=postgres://conusai:conusai@localhost:5432/conusai \
	  cargo sqlx migrate run --source apps/backend/crates/common/migrations

## Revert last sqlx migration
db-migrate-revert:
	DATABASE_URL=postgres://conusai:conusai@localhost:5432/conusai \
	  cargo sqlx migrate revert --source apps/backend/crates/common/migrations

## Refresh the sqlx offline query cache
sqlx-prepare:
	DATABASE_URL=postgres://conusai:conusai@localhost:5432/conusai \
	  cargo sqlx prepare --workspace

## Run all E2E tests (web + iOS emulation)
test-e2e: test-e2e-web test-e2e-ios

## Run web E2E tests only
test-e2e-web:
	pnpm e2e:web

## Run iOS mobile-web E2E tests (Playwright device emulation)
test-e2e-ios:
	pnpm e2e:ios

## Run macOS shell E2E tests (requires tauri-driver running)
## Usage: make test-e2e-shell  (auto-starts tauri-driver)
test-e2e-shell:
	@BINARY="apps/browser-shell/src-tauri/target/aarch64-apple-darwin/debug/browser-shell"; \
	if [ ! -f "$$BINARY" ]; then \
	  echo "Building browser-shell debug binary..."; \
	  cd apps/browser-shell && pnpm tauri build --debug; \
	fi; \
	tauri-driver --port 9515 --binary "$$BINARY" & \
	DRIVER_PID=$$!; \
	sleep 3; \
	TAURI_WEBDRIVER_URL="ws://127.0.0.1:9515" CONUSAI_E2E=1 pnpm e2e:shell; \
	kill $$DRIVER_PID

## Start tauri-driver in the foreground (for manual Playwright runs)
## Prints ws endpoint, then blocks. Ctrl+C to stop.
start-tauri-driver:
	@BINARY="apps/browser-shell/src-tauri/target/aarch64-apple-darwin/debug/browser-shell"; \
	echo "WebDriver endpoint: ws://127.0.0.1:9515"; \
	tauri-driver --port 9515 --binary "$$BINARY"

## Start the indexer only (no gateway)
indexer-dev:
	WORKSPACES_ROOT=./workspaces cargo run -p agent-gateway -- --indexer-only
