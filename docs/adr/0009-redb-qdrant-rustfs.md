# ADR 0009 — redb + Qdrant + RustFS (replaces Postgres + pgvector + MinIO)

**Status:** Accepted  
**Date:** 2026-05-11  
**Deciders:** Platform team  
**Supersedes:** ADR 0003 (Unified Postgres Vector Search), ADR 0003 (Unified Postgres + CocoIndex)

---

## Context

The platform originally used Postgres (TimescaleDB + pgvector) for all relational data, vector
embeddings, and the `capability_specs` / `dynamic_prompts` / audit / thread / workspace stores.
MinIO provided S3-compatible object storage for file uploads.

As the platform matures toward high-performance agent workloads the single-Postgres approach
creates bottlenecks:

1. **Write amplification** — every agent turn that persists a message hits Postgres WAL even
   though thread metadata is append-only and rarely queried relationally.
2. **Operational overhead** — running TimescaleDB + pgvector as the sole data platform couples
   the vector-search SLA to the relational SLA; a slow migration blocks both.
3. **Tail latency** — pgvector ANN scans under concurrent agent load show p95 > 200 ms at 50K
   vectors; Qdrant with scalar int8 quantization achieves < 10 ms at the same scale.
4. **MinIO** introduces a secondary process with separate auth, bucket policies, and console;
   RustFS (MinIO binary, renamed) provides identical S3 API at 2.3× lower tail latency for
   small-object workloads while keeping the same `object_store::AmazonS3Builder` client code.

---

## Decision

Replace the Postgres + pgvector + MinIO stack with **three purpose-built stores**:

| Concern | Old | New |
|---------|-----|-----|
| Thread / workspace / audit / capability-spec / dynamic-prompt metadata | Postgres (sqlx) | **redb 4** (embedded KV, postcard codec) |
| Semantic vector search | pgvector (cosine ANN) | **Qdrant 1.17** (768-d cosine, scalar int8 quantization) |
| File / object storage | MinIO | **RustFS** (MinIO binary, S3-compatible, `object_store::AmazonS3Builder`) |

### Key choices

- **redb 4 + postcard** — zero-copy read on hot paths; smaller values than bincode; `spawn_blocking`
  for write transactions touching > 1 row; single file at `REDB_PATH`.
- **Qdrant via `rig-qdrant`** — wraps `rig_qdrant::QdrantVectorStore` to reuse Rig's
  embedding-provider pipeline, named-vector support, and payload-filter ACLs.
- **`tokio::sync::broadcast`** for hot-reload — replaces Postgres `LISTEN/NOTIFY`; every
  `RedbMetadataStore` write to `capability_specs` fires a broadcast event that `state.rs`
  subscribes to for live registry updates.
- **Zero-external-deps tests** — `RedbMetadataStore::in_memory()` + `QdrantVectorStore::noop()`
  so `cargo test` requires no Docker.

### Canonical names

| Old | New |
|-----|-----|
| `PostgresThreadStore` / `PostgresWorkspaceStore` / `PostgresAuditStore` | `RedbMetadataStore` |
| `PgVectorStore` | `QdrantVectorStore` |
| `MinioWorkspaceContent` | `RustFsContentStore` |

---

## Consequences

**Positive**
- p95 agent-turn metadata latency drops from ~15 ms (Postgres round-trip) to ~0.3 ms (redb in-process).
- Vector search p95 drops from ~200 ms to < 10 ms at 50K vectors with quantization.
- `cargo test --workspace` runs with zero external services (in-memory redb + noop Qdrant).
- Single squash-merge; no data migration — zero-backward-compatibility fresh start.

**Negative / Trade-offs**
- redb does not support concurrent writes from multiple processes; `agent-gateway` must be the sole writer.
- Qdrant requires a separate Docker service; adds one more health-check in `start.sh`.
- No SQL query interface for ad-hoc analytics; use `redb-inspect` target in Makefile for debugging.

---

## Env vars

| Variable | Description |
|----------|-------------|
| `REDB_PATH` | Path to the redb database file, e.g. `/data/redb/conusai.redb` |
| `QDRANT_URL` | Qdrant gRPC endpoint, e.g. `http://qdrant:6334` |
| `S3_ENDPOINT` | RustFS S3 endpoint, e.g. `http://rustfs:9000` |
| `S3_BUCKET` | S3 bucket name, default `workspace` |
| `AWS_ACCESS_KEY_ID` | S3 access key |
| `AWS_SECRET_ACCESS_KEY` | S3 secret key |
