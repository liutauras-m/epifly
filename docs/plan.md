# ConusAI High-Performance Migration Plan

**Source spec:** [docs/tasks/hi-performance-task.md](docs/tasks/hi-performance-task.md)

**Goal:** Aggressive, **zero-backward-compatibility** migration from Postgres + MinIO to **redb + Qdrant + RustFS + Marker**. Every Postgres-, sqlx-, rig-postgres-, pgvector-, MinIO- and minio-init-touching file is rewritten or deleted. No feature flags, no dual-stack, no migrations from old data — fresh start.

**Date:** 2026-05-11
**Scope:** All Rust crates under [apps/backend](apps/backend), Docker infra, env config, docs, e2e tests.

---

## 0. Guiding Rules

1. **Delete first, then implement.** No `cfg!`, no `if storage_backend == "postgres"`, no compatibility shims.
2. **Canonical names** (per [hi-performance-task.md](docs/tasks/hi-performance-task.md#canonical-name-changes)):
   - `PostgresThreadStore` + `PostgresWorkspaceStore` + `PostgresAuditStore` + dynamic-prompt/capability-spec PG access → **`RedbMetadataStore`**.
   - `PgVectorStore` → **`QdrantVectorStore`**.
   - `MinioWorkspaceContent` → **`RustFsContentStore`**.
3. **One concrete impl per trait** wired in `AppState::from_env`; no factories selecting backends.
4. **No SQL strings anywhere** (`grep -RIn "sqlx::query\|SELECT \|INSERT INTO\|LISTEN \|NOTIFY "` in `apps/backend` must return zero hits after Phase 4).
5. **All work happens on a feature branch** `feat/hpm-redb-qdrant-rustfs`. Single squash-merge at the end.

### 0.0 2026 research alignment (May 2026)

The stack below is the industry-converged 2026 reference for Rust agent backends — adopted in Rig 0.36 + rig-qdrant 0.2.5, Agentor, and Cortex Memory. Key idiomatic choices baked into this plan:

- **redb 4.x** with **postcard** value codec — smaller/faster than bincode; zero-copy reads on hot paths. `spawn_blocking` for any txn touching >1 row.
- **Qdrant 1.17+** via **`rig_qdrant::QdrantVectorStore`** wrapper (not raw `qdrant-client`) — gets named vectors, scalar int8 quantization, payload-filter ACLs, and future hybrid search for free.
- **RustFS** behind unchanged `object_store::aws::AmazonS3Builder` — 2.3× lower tail latency vs MinIO at small-object workloads.
- **Marker** behind an `#[async_trait] MarkerClient` — future WASM/browser-shell swap-in.
- **Zero-external-deps tests**: `RedbMetadataStore::test()` pairs in-memory redb with `Qdrant::new_in_memory()` so unit + integration suites run with no Docker.

---

## Phase 0 — Infra reset (Docker + env)

### 0.1 Replace [docker-compose.yml](docker-compose.yml)
Delete services: `postgres`, `minio`, `minio-init`. Replace with the 4-service stack from [hi-performance-task.md](docs/tasks/hi-performance-task.md) (`rustfs`, `qdrant`, `marker-api`, `agent-gateway`). Remove `postgres_data` and `minio_data` named volumes; add `rustfs_data`, `qdrant_data`, `redb_data`. Drop `infra` / `full` profiles — single profile, all services start by default.

### 0.2 Delete Postgres init scripts
- `rm -rf` [docker/init/](docker/init) (both [01-extensions.sql](docker/init/01-extensions.sql) and [02-schema.sql](docker/init/02-schema.sql)).

### 0.3 Update env files
- `.env.example`, `.env`, `.env.local`: remove `DATABASE_URL`, `POSTGRES_*`, `MINIO_*`. Add `QDRANT_URL`, `S3_ENDPOINT=http://rustfs:9000`, `S3_BUCKET=workspace`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `MARKER_URL`, `REDB_PATH=/data/redb/conusai.redb`.
- [start.sh](start.sh), [stop.sh](stop.sh), [apps/backend/start.sh](apps/backend/start.sh), [apps/backend/start-verify.sh](apps/backend/start-verify.sh), [apps/backend/stop.sh](apps/backend/stop.sh): remove all `pg_isready`, `mc alias`, `wait-for-postgres` logic; replace with `curl http://qdrant:6333/healthz` + `curl http://rustfs:9000/minio/health/live`.

### 0.4 Update [Makefile](Makefile) and [justfile](justfile)
Remove `db-migrate`, `db-reset`, `psql`, `minio-mc` targets. Add `redb-inspect`, `qdrant-collections`, `rustfs-mc` helpers.

---

## Phase 1 — Cargo workspace cleanup

### 1.1 Workspace [Cargo.toml](Cargo.toml)
**Remove**:
```toml
sqlx         = { version = "0.8", features = [...] }
rig-postgres = { version = "0.2.5" }
```
**Add**:
```toml
redb          = "4.1"
postcard      = { version = "1", features = ["alloc"] }   # value codec for redb (smaller + faster than bincode, zero-copy reads)
qdrant-client = { version = "1", default-features = false, features = ["serde"] }
rig-qdrant    = "0.2.5"                                    # pin to latest as of 2026-05
```
Keep `object_store` (still used by `RustFsContentStore` via `AmazonS3Builder`).

### 1.2 Per-crate Cargo manifests
- `apps/backend/crates/common/Cargo.toml`: drop `sqlx`; add `redb`.
- `apps/backend/crates/agent-core/Cargo.toml`: drop `sqlx`, `rig-postgres`; add `redb`, `qdrant-client`, `rig-qdrant`.
- `apps/backend/crates/agent-gateway/Cargo.toml`: drop `sqlx`.
- `apps/backend/crates/jobs/Cargo.toml`: drop `sqlx`.
- Remove every `features = ["postgres", "sqlite"]` block.

### 1.3 Compile gate
After this phase the workspace **must not compile**. That is intentional — the next phase fixes every error.

---

## Phase 2 — Delete Postgres/MinIO source files

Hard-delete (no rename, no archive):

| Path | Reason |
|---|---|
| [apps/backend/crates/common/migrations/](apps/backend/crates/common/migrations) (entire dir, all 9 `.up.sql` files) | sqlx migrations — gone |
| [apps/backend/crates/common/src/db.rs](apps/backend/crates/common/src/db.rs) | `create_pool` / `PgPool` re-export |
| [apps/backend/crates/agent-core/src/memory/postgres_thread_store.rs](apps/backend/crates/agent-core/src/memory/postgres_thread_store.rs) | replaced by `RedbMetadataStore` |
| [apps/backend/crates/agent-core/src/memory/postgres_workspace_store.rs](apps/backend/crates/agent-core/src/memory/postgres_workspace_store.rs) | ↑ |
| [apps/backend/crates/agent-core/src/memory/postgres_audit_store.rs](apps/backend/crates/agent-core/src/memory/postgres_audit_store.rs) | ↑ |
| [apps/backend/crates/agent-core/src/memory/minio_workspace_content.rs](apps/backend/crates/agent-core/src/memory/minio_workspace_content.rs) | replaced by `RustFsContentStore` |
| [apps/backend/crates/agent-core/src/vector_store/postgres.rs](apps/backend/crates/agent-core/src/vector_store/postgres.rs) | replaced by `QdrantVectorStore` |

Update [apps/backend/crates/agent-core/src/memory/mod.rs](apps/backend/crates/agent-core/src/memory/mod.rs) and [apps/backend/crates/agent-core/src/vector_store/mod.rs](apps/backend/crates/agent-core/src/vector_store/mod.rs) to remove the corresponding `mod` / `pub use` lines (will re-export new modules in Phase 3).

---

## Phase 3 — Implement new stores (`crates/agent-core/src/store/`)

Create a new top-level module `agent-core::store` to host the three concrete stores. Old `memory/` and `vector_store/` modules are kept only for trait definitions that still live in `common::memory::store` and `common::audit`.

### 3.1 `store/redb_metadata.rs` — `RedbMetadataStore`

Single struct holding an `Arc<redb::Database>`. Implements **all** of:
- `common::memory::store::ThreadStore`
- `common::memory::store::WorkspaceStore`
- `common::audit::AuditStore`
- a new internal `DynamicPromptStore` trait (extracted from [chains/dynamic_prompt.rs](apps/backend/crates/agent-core/src/chains/dynamic_prompt.rs))
- a new internal `CapabilitySpecStore` trait (extracted from [capabilities/providers/capability_spec.rs](apps/backend/crates/agent-core/src/capabilities/providers/capability_spec.rs))

Tables (typed `redb::TableDefinition<'static, K, V>` with **postcard**-serialized values — smaller + faster than bincode in 2026 agent-metadata workloads, with zero-copy deserialization for read-heavy paths):

| Table | Key | Value |
|---|---|---|
| `threads` | `&str` (thread_id) | `Thread` |
| `messages` | `(&str, u64)` (thread_id, seq) | `Message` |
| `idx_threads_by_tenant` | `(&str, &str)` (tenant, thread_id) | `()` |
| `workspace_nodes` | `&str` (node_id) | `WorkspaceNode` |
| `idx_nodes_by_tenant_parent` | `(&str, Option<&str>, &str)` (tenant, parent, name) | `&str` (node_id) |
| `idx_nodes_by_path` | `(&str, &str)` (tenant, virtual_path) | `&str` |
| `audit_events` | `(&str, i64, &str)` (tenant, ts_micros, id) | `AuditEvent` |
| `dynamic_prompts` | `(&str, u32)` (name, version) | `PromptVersion` |
| `capability_specs` | `(&str, &str)` (namespace, tool_name) | `CapabilitySpecRow` |

Rules:
- Every mutation uses a single `WriteTransaction` that touches the primary table + every relevant index in one commit (atomic).
- No background flush thread — redb is sync; wrap calls in `tokio::task::spawn_blocking` for endpoints that touch >1 row.
- Constructor: `RedbMetadataStore::open(path: impl AsRef<Path>) -> anyhow::Result<Arc<Self>>`.
- Test constructors: `RedbMetadataStore::in_memory()` using `redb::Builder::new().create_with_backend(redb::backends::InMemoryBackend::new(), ...)`, plus `RedbMetadataStore::test()` (in `store/test.rs`) that returns the in-memory store **paired with a `QdrantVectorStore` running against `Qdrant::new_in_memory()`** so integration tests need zero external services (matches 2026 Rig/Agentor convention).

### 3.2 `store/qdrant_vector.rs` — `QdrantVectorStore`

- **Wraps `rig_qdrant::QdrantVectorStore`** (one inner per collection) instead of driving `qdrant-client` directly. This reuses Rig's embedding-provider pipeline, named-vector support, and forthcoming hybrid-search primitives for free; the semantic router collapses to a single `vector_store.top_n(query, k)` call.
- Two collections, created lazily on first write via the underlying `qdrant_client::Qdrant`: `capability_embeddings` (768-d cosine) and `content_embeddings` (768-d cosine, with payload index on `tenant_id`, `node_id`, `namespace`). Enable **scalar int8 quantization** on both — 2026 community canon at this scale.
- Our struct exposes thin conversion methods that map `rig_qdrant` results into our `CapabilityHit` / `ContentHit` DTOs, so existing call sites in [capabilities/semantic_router.rs](apps/backend/crates/agent-core/src/capabilities/semantic_router.rs) and [indexing/coco_indexer.rs](apps/backend/crates/agent-core/src/indexing/coco_indexer.rs) need only a type swap:
  - `upsert_capability(id, embedding, namespace, tags, content, metadata)`
  - `upsert_content(node_id, chunk_idx, embedding, tenant_id, content, ...)`
  - `delete_content_by_node(tenant_id, node_id)`
  - `search_capabilities(query_emb, top_k, namespace_filter) -> Vec<CapabilityHit>`
  - `search_content(query_emb, top_k, tenant_id) -> Vec<ContentHit>`
- Use Qdrant payload filters (`Filter::must([Condition::matches("tenant_id", tenant)])`) for ACL — do **not** hand-roll.
- Constructors: `connect(url) -> Self`, `in_memory()` (uses `Qdrant::new_in_memory()` for unit/integration tests), and `noop()` for `CONUSAI_TEST_MODE=1` smoke paths (returns empty vecs, errors on writes).

### 3.3 `store/rustfs_content.rs` — `RustFsContentStore`

- Wraps `Arc<dyn ObjectStore>` built via `object_store::aws::AmazonS3Builder`.
- Implements `WorkspaceContentStore`. Logic identical to old [minio_workspace_content.rs](apps/backend/crates/agent-core/src/memory/minio_workspace_content.rs); only the constructor + module/struct names change.
- Constructor `RustFsContentStore::from_env(cfg: &StorageConfig) -> anyhow::Result<Arc<Self>>` reading `S3_ENDPOINT`, `S3_BUCKET`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY` (already exposed via `figment` in `common::config`). Replaces `init_file_store()` in [agent-gateway/src/state.rs](apps/backend/crates/agent-gateway/src/state.rs).

### 3.4 `store/marker.rs` — `MarkerClient`

- Thin `reqwest` client around `MARKER_URL` defined behind an `#[async_trait] trait MarkerClient: Send + Sync + 'static` so a future WASM/browser-shell impl (or in-process Marker) can be swapped without touching consumers.
- Single method `async fn pdf_to_markdown(&self, bytes: Bytes) -> anyhow::Result<String>`.
- Injected as `Arc<dyn MarkerClient>` into `WorkspaceIndexer` (Phase 4) and `ArtifactBridge` (when `mime_type == "application/pdf"`).

### 3.5 Module layout & wiring

```text
apps/backend/crates/agent-core/src/store/
├── mod.rs                  # pub use RedbMetadataStore, QdrantVectorStore, RustFsContentStore, MarkerClient
├── redb_metadata.rs        # all *Store traits + internal DynamicPromptStore + CapabilitySpecStore
├── qdrant_vector.rs        # wraps rig_qdrant::QdrantVectorStore
├── rustfs_content.rs
├── marker.rs
└── test.rs                 # shared test constructors (in-memory redb + in-memory Qdrant)
```

In [apps/backend/crates/agent-core/src/lib.rs](apps/backend/crates/agent-core/src/lib.rs):
```rust
pub mod store;
pub use store::{RedbMetadataStore, QdrantVectorStore, RustFsContentStore, MarkerClient};
```

`memory/` and `vector_store/` modules remain only as homes for the **trait** definitions re-exported from `common` — strict SRP.

---

## Phase 4 — Rewrite consumers

### 4.1 [apps/backend/crates/common/src/config/mod.rs](apps/backend/crates/common/src/config/mod.rs)
Remove every `database_url`, `postgres_*`, `minio_*` field. New `StorageConfig`:
```rust
pub struct StorageConfig {
    pub redb_path: PathBuf,        // REDB_PATH
    pub qdrant_url: String,        // QDRANT_URL
    pub s3_endpoint: String,       // S3_ENDPOINT
    pub s3_bucket: String,         // S3_BUCKET
    pub s3_access_key: String,
    pub s3_secret_key: String,
    pub marker_url: String,        // MARKER_URL
}
```

### 4.2 [apps/backend/crates/agent-gateway/src/state.rs](apps/backend/crates/agent-gateway/src/state.rs)
- Drop fields: `pool: Option<PgPool>`, `vector_store: Arc<PgVectorStore>` → replaced by `Arc<QdrantVectorStore>`. Remove `Option` wrappers on `artifact_bridge` and `capability_spec_factory` — they are always present now.
- `AppState::from_env`:
  1. Build `metadata = RedbMetadataStore::open(cfg.redb_path)?`.
  2. Build `vectors  = QdrantVectorStore::connect(&cfg.qdrant_url).await?`.
  3. Build `content  = RustFsContentStore::from_env(&cfg)?`.
  4. Build `marker   = MarkerClient::new(&cfg.marker_url)`.
  5. `thread_store / workspace_store / audit_store` all = `Arc::clone(&metadata) as Arc<dyn _>`.
  6. Drop the entire `tokio::spawn` LISTEN/NOTIFY block (Phase 4.5 replaces it).
- `with_in_memory_stores`: use `RedbMetadataStore::in_memory()` + `QdrantVectorStore::noop()` + `NoopWorkspaceContent`.

### 4.3 [apps/backend/crates/agent-gateway/src/main.rs](apps/backend/crates/agent-gateway/src/main.rs)
Remove the Postgres `LISTEN` startup task and any `pool` health probe. Add Qdrant + RustFS readiness checks (single `tokio::join!` of two HTTP HEADs) before `axum::serve`.

### 4.4 [apps/backend/crates/agent-core/src/capabilities/providers/dynamic_prompt.rs](apps/backend/crates/agent-core/src/capabilities/providers/dynamic_prompt.rs) and [chains/dynamic_prompt.rs](apps/backend/crates/agent-core/src/chains/dynamic_prompt.rs)
Replace `pool: PgPool` with `metadata: Arc<RedbMetadataStore>`. Loads/versions/active-pointer use `dynamic_prompts` redb table.

### 4.5 [apps/backend/crates/agent-core/src/capabilities/providers/capability_spec.rs](apps/backend/crates/agent-core/src/capabilities/providers/capability_spec.rs)
- Replace `pool: PgPool` + `vector_store: Arc<PgVectorStore>` with `metadata: Arc<RedbMetadataStore>` + `vectors: Arc<QdrantVectorStore>`.
- `load_batch`: stream rows from redb `capability_specs` table; embed in batches via `embedder`; upsert into Qdrant `capability_embeddings`.
- `reload_one(namespace, tool_name)`: single redb read + single Qdrant upsert.
- **Hot-reload**: replace PG `LISTEN/NOTIFY` with an in-process `tokio::sync::broadcast::Sender<(String,String)>` exposed on `RedbMetadataStore`. Every `capability_specs` write fires `tx.send(...)`. The subscriber task in `state.rs` calls `factory.reload_one(...)` exactly as today.

### 4.6 [apps/backend/crates/agent-core/src/capabilities/registry.rs](apps/backend/crates/agent-core/src/capabilities/registry.rs)
`with_all_factories(llm, metadata, vectors)` — no `Option<PgPool>`. Discovery in [discovery.rs](apps/backend/crates/agent-core/src/capabilities/discovery.rs) unaffected (filesystem-only).

### 4.7 [apps/backend/crates/agent-core/src/capabilities/semantic_router.rs](apps/backend/crates/agent-core/src/capabilities/semantic_router.rs)
Type-swap `Arc<PgVectorStore>` → `Arc<QdrantVectorStore>`. Method names already match.

### 4.8 [apps/backend/crates/agent-core/src/capabilities/admin.rs](apps/backend/crates/agent-core/src/capabilities/admin.rs)
All CRUD goes through `RedbMetadataStore::{put,get,delete}_capability_spec` → which fires the broadcast event for hot-reload. Drop all `sqlx::query` calls.

### 4.9 [apps/backend/crates/agent-core/src/indexing/coco_indexer.rs](apps/backend/crates/agent-core/src/indexing/coco_indexer.rs)
Replace `pool: PgPool` + `vector_store: Arc<PgVectorStore>` with `metadata: Arc<RedbMetadataStore>` + `vectors: Arc<QdrantVectorStore>` + `marker: Arc<MarkerClient>`. PDF branch:
```rust
let md = self.marker.pdf_to_markdown(bytes).await?;
self.content.write(tenant, virtual_path, &md).await?;
self.embed_and_upsert(node_id, &md).await?;   // → Qdrant
```
Drop `sha2`/checksum table — store last-indexed hash in redb `idx_content_hash` table (tenant, virtual_path) → blake3.

### 4.10 [apps/backend/crates/agent-core/src/bridge/artifact_bridge.rs](apps/backend/crates/agent-core/src/bridge/artifact_bridge.rs)
- `new(metadata: Arc<RedbMetadataStore>, content: Arc<RustFsContentStore>, marker: Arc<MarkerClient>)`.
- Replace the `INSERT INTO workspace_nodes` SQL with `metadata.create_file_node(...)`.
- For PDF artifacts, additionally call Marker → write `.md` sibling node.

### 4.11 [apps/backend/crates/agent-core/src/realtime/mod.rs](apps/backend/crates/agent-core/src/realtime/mod.rs)
- Remove `pool: PgPool` and `LISTEN capability_specs_changed` task.
- Subscriber sources: in-process `broadcast` channels published by `RedbMetadataStore` for capability-spec / workspace-node / thread-message changes.
- Public API (`subscribe_capability_spec_changes`, `subscribe_workspace_changes`, …) unchanged → routes/WS layer untouched.

### 4.12 [apps/backend/crates/agent-gateway/src/routes/files.rs](apps/backend/crates/agent-gateway/src/routes/files.rs)
- Replace MinIO presign helpers with the RustFS S3 endpoint (URL only changes; `object_store` already supports it).
- Drop `MINIO_*` env probes; read from `AppState::storage_cfg`.

### 4.13 [apps/backend/crates/agent-gateway/src/routes/realtime.rs](apps/backend/crates/agent-gateway/src/routes/realtime.rs)
No code change beyond adapting to the trimmed `RealtimeService::new(metadata)` constructor.

### 4.14 Jobs crate ([apps/backend/crates/jobs](apps/backend/crates/jobs))
- [src/context.rs](apps/backend/crates/jobs/src/context.rs): replace `PgPool` and `minio_endpoint` with `Arc<RedbMetadataStore>` + `Arc<RustFsContentStore>`.
- [src/jobs/audit_log_cleanup.rs](apps/backend/crates/jobs/src/jobs/audit_log_cleanup.rs): rewrite using `metadata.prune_audit_before(ts)` (new method backed by redb range-delete).
- [src/jobs/capability_health_check.rs](apps/backend/crates/jobs/src/jobs/capability_health_check.rs): swap PG ping for `qdrant.health()` and RustFS `HEAD`.
- [src/jobs/video_transcription.rs](apps/backend/crates/jobs/src/jobs/video_transcription.rs): swap S3 client to `RustFsContentStore`.

### 4.15 [services/current-time](services/current-time) and [apps/backend/capabilities/file-storage/capability.toml](apps/backend/capabilities/file-storage/capability.toml)
Update env keys (`S3_ENDPOINT`, `S3_BUCKET`) — no logic change.

---

## Phase 5 — Tests

### 5.1 Unit tests
- Delete `#[sqlx::test]` annotations across the workspace; replace with plain `#[tokio::test]` using `RedbMetadataStore::in_memory()`.
- Delete every `wiremock` setup that mocks Postgres responses.

### 5.2 Integration tests
- [apps/backend/crates/agent-gateway/tests/remote_mcp_e2e.rs](apps/backend/crates/agent-gateway/tests/remote_mcp_e2e.rs) and siblings: switch to `AppState::with_in_memory_stores()`.
- [apps/backend/crates/jobs/tests/executor_tests.rs](apps/backend/crates/jobs/tests/executor_tests.rs): update `JobContext::test()` builder.

### 5.3 E2E
- [playwright.config.ts](playwright.config.ts) + [e2e/](e2e): no app-side changes; only the `docker compose up` preflight switches services. Update [e2e/fixtures/seed-workspace.ts](e2e/fixtures/seed-workspace.ts) — drop SQL seed; use the new `POST /admin/seed` endpoint that calls `RedbMetadataStore` directly.

### 5.4 Backend evals ([apps/backend/evals](apps/backend/evals))
Audit any `sqlx::PgPool::connect` usage; replace with the in-memory metadata store.

---

## Phase 6 — Documentation & ADRs

- Mark superseded: [docs/adr/0003-unified-postgres-vector-search.md](docs/adr/0003-unified-postgres-vector-search.md), [docs/adr/0003-unified-postgres-cocoindex.md](docs/adr/0003-unified-postgres-cocoindex.md). Add front-matter `status: superseded by ADR-0009`.
- Update [docs/adr/0004-semantic-capability-router-and-dynamic-prompts.md](docs/adr/0004-semantic-capability-router-and-dynamic-prompts.md) — replace LISTEN/NOTIFY paragraph with in-process broadcast.
- New ADR `docs/adr/0009-redb-qdrant-rustfs.md` summarising the decision (one page).
- Rewrite storage / runtime sections of [docs/arch.md](docs/arch.md).
- Rewrite Postgres/MinIO checklists in [docs/verify/verify.md](docs/verify/verify.md) → redb file size check, Qdrant collection counts, RustFS bucket listing.
- Update [README.md](README.md) "Quickstart" — no `psql`, no `mc`.
- Delete or rewrite obsolete task notes referencing PG/MinIO under [docs/tasks/](docs/tasks).

---

## Phase 7 — Observability

Add OTel meter instruments in `agent-core`:
- `storage.redb.txn.duration` (histogram, ms, label `op = read|write`)
- `storage.redb.bytes` (gauge, file size on disk)
- `vector.qdrant.search.duration` (histogram, label `collection`)
- `vector.qdrant.upsert.batch_size` (histogram)
- `storage.rustfs.request.duration` (histogram, label `op = get|put|delete|head`)

Remove old `db.postgres.*` and `storage.minio.*` instruments wherever defined.

---

## Phase 8 — Cutover & verification

1. Drop the dev branch onto a clean checkout — no data migration code, no Postgres dump (zero-backward-compat).
2. `docker compose down -v` then `docker compose up -d --build` against the new compose file.
3. `cargo build --workspace` — must succeed with zero `sqlx`, `rig_postgres`, `MinioWorkspaceContent`, `PgVectorStore` references.
4. Run `cargo test --workspace`.
5. Run full Playwright suite: `pnpm e2e`.
6. Smoke: upload a PDF → verify Marker conversion → semantic search hits → realtime WS sees `capability_specs_changed`-equivalent broadcast → browser-shell renders the new file.
7. Quick load test (k6 or `oha`) — 100 concurrent users, 60 s — assert p95 agent-turn latency < 100 ms (per [hi-performance-task.md](docs/tasks/hi-performance-task.md) target).

---

## Phase 9 — Cleanup verification (CI gates)

Add to CI (`.github/workflows/ci.yml` or equivalent):

```bash
# 1. No SQL in source
! rg -nP '(sqlx::|PgPool|rig_postgres|pgvector|MINIO_|MinioWorkspaceContent|PgVectorStore|Postgres(Thread|Workspace|Audit)Store)' apps/backend

# 2. No old infra references
! rg -n 'postgres:|minio:|minio-init:' docker-compose.yml

# 3. No leftover SQL files
! find apps/backend -name '*.sql' -print -exec false {} +
```

A red CI on any of these blocks merge.

---

## File-level change ledger

| Action | Count | Examples |
|---|---|---|
| Delete | 16 | 9× SQL migrations, 3× `postgres_*_store.rs`, `minio_workspace_content.rs`, `vector_store/postgres.rs`, `common/src/db.rs`, `docker/init/*.sql` |
| Create | 6 | `store/{redb_metadata,qdrant_vector,rustfs_content,marker,test}.rs`, `docs/adr/0009-redb-qdrant-rustfs.md` |
| Rewrite | ~15 | `state.rs`, `main.rs`, `realtime/mod.rs`, `coco_indexer.rs`, `artifact_bridge.rs`, `semantic_router.rs`, `dynamic_prompt.rs`, `capability_spec.rs`, `admin.rs`, `routes/files.rs`, `jobs/context.rs` + 3 jobs, `common/config/mod.rs`, `docker-compose.yml` |
| Touch (small) | ~10 | `Cargo.toml` × 5, `mod.rs` re-exports × 2, `arch.md`, `verify.md`, `README.md` |

---

## Suggested execution order (single working branch)

1. **PR 1 — infra + config** (Phase 0 + 1.1): compose file, env, workspace `Cargo.toml`. Workspace will not compile yet.
2. **PR 2 — new stores** (Phase 3): land `agent-core::store::*` modules with full tests in isolation.
3. **PR 3 — wire-through + delete** (Phase 2 + 4): single big PR that flips every consumer and deletes the old files. Compiles green.
4. **PR 4 — tests + docs** (Phases 5, 6, 7): green CI + new gates.
5. **PR 5 — cutover** (Phase 8 + 9): bump compose tags, ship.

Each PR is independently reviewable but only PR 3 onward yields a runnable backend — acceptable because there is no production we are preserving.
