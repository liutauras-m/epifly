# Makefile — convenience wrappers for the conusai-platform monorepo

.PHONY: verify fmt lint test build clean

## Run full verification (fmt, clippy, tests)
verify:
	cd apps/backend && cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace

## Format all Rust code
fmt:
	cd apps/backend && cargo fmt --all

## Run clippy
lint:
	cd apps/backend && cargo clippy --workspace --all-targets -- -D warnings

## Run tests
test:
	cd apps/backend && cargo test --workspace

## Production build
build:
	cd apps/backend && cargo build --release --bin agent-gateway

## Clean build artefacts
clean:
	rm -rf apps/backend/target
