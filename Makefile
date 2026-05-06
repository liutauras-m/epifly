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

## Start the indexer only (no gateway)
indexer-dev:
	WORKSPACES_ROOT=./workspaces cargo run -p agent-gateway -- --indexer-only
