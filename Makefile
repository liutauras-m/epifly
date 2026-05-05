# Makefile — convenience wrappers for the conusai-platform monorepo

.PHONY: verify fmt lint test build clean

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
