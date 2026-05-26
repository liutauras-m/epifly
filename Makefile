# Makefile — convenience wrappers for the conusai-platform monorepo

.PHONY: verify fmt lint test build clean \
        test-e2e test-e2e-web test-e2e-ios test-e2e-shell start-tauri-driver \
        verify-no-dead-deps verify-routes-doc verify-no-commented-code ci

## Run full verification (fmt, clippy, dead-code, routes, tests)
verify: lint verify-no-dead-deps verify-no-commented-code test

## CI gate — all blocking checks in sequence
ci: fmt verify-no-dead-deps lint test verify-routes-doc verify-no-commented-code

## Format all Rust code
fmt:
	cargo fmt --all

## Clippy with full deny list (todo/unimplemented/dbg_macro also denied)
lint:
	cargo clippy --workspace --all-targets -- \
	  -D warnings \
	  -D dead_code \
	  -D clippy::todo \
	  -D clippy::unimplemented \
	  -D clippy::dbg_macro \
	  -D clippy::print_stdout

## Run tests
test:
	cargo test --workspace

## Fail if any unused workspace dep is detected (requires cargo-machete)
verify-no-dead-deps:
	@command -v cargo-machete >/dev/null 2>&1 || cargo install cargo-machete --quiet
	cargo machete

## Fail if route table has drifted from the generated snapshot
verify-routes-doc:
	@mkdir -p docs
	./scripts/dump-routes.sh docs/_routes.generated.md
	node scripts/verify-route-wiring.mjs
	@if [ -f docs/_routes.expected.md ]; then \
	  diff -u docs/_routes.expected.md docs/_routes.generated.md || \
	    (echo "ERROR: route table drift — run: cp docs/_routes.generated.md docs/_routes.expected.md" && exit 1); \
	else \
	  echo "INFO: no expected snapshot yet — creating docs/_routes.expected.md"; \
	  cp docs/_routes.generated.md docs/_routes.expected.md; \
	fi

## Fail if any TODO/FIXME/XXX comment is older than 30 days (requires git blame)
verify-no-commented-code:
	@echo "Checking for stale TODO/FIXME/XXX comments (>30 days)..."
	@CUTOFF=$$(date -v-30d +%Y-%m-%d 2>/dev/null || date -d '30 days ago' +%Y-%m-%d); \
	FOUND=0; \
	while IFS= read -r match; do \
	  FILE=$$(echo "$$match" | cut -d: -f1); \
	  LINE=$$(echo "$$match" | cut -d: -f2); \
	  BLAME_DATE=$$(git blame -L "$$LINE,$$LINE" --date=short -- "$$FILE" 2>/dev/null | grep -oE '[0-9]{4}-[0-9]{2}-[0-9]{2}' | head -1); \
	  if [ -n "$$BLAME_DATE" ] && [ "$$BLAME_DATE" \< "$$CUTOFF" ]; then \
	    echo "  STALE ($$BLAME_DATE): $$FILE:$$LINE"; \
	    FOUND=$$((FOUND+1)); \
	  fi; \
	done < <(grep -rn --include="*.rs" -E '//\s*(TODO|FIXME|XXX)\b' apps/backend/crates/ 2>/dev/null || true); \
	if [ "$$FOUND" -gt 0 ]; then \
	  echo "ERROR: $$FOUND stale comment(s) found. Resolve or add a tracked issue URL."; \
	  exit 1; \
	fi; \
	echo "OK: no stale comments found."

## Production build
build:
	cargo build --release --bin agent-gateway

## Clean build artefacts
clean:
	rm -rf apps/backend/target

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
