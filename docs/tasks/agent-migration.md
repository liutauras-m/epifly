**ConusAI Platform – Aggressive Nuclear Refactoring Plan v0.3.1**  
**Stack: Postgres 18 + pgvector + pgvectorscale + CocoIndex + rig-postgres (May 2026 canonical)**

**Verdict**: Full rip-and-replace. Zero backward compatibility, zero migration paths, zero legacy code. We are burning the Qdrant bridge, deleting every `ToolProvider` reference, and enforcing v0.3 canonical architecture (CapabilityProvider-first, Rig.rs 0.36+ native, real-FS source of truth). This is the cleanest, most maintainable path for 2026 production agent platforms.

**Why this is the right aggressive move**  
- Unified ACID DB (metadata + vectors + workspaces + audit) eliminates dual-DB complexity.  
- CocoIndex + Tree-sitter gives true incremental, language-aware indexing on real POSIX FS — exactly what long-horizon agents need.  
- rig-postgres + pgvectorscale delivers 16–28× better perf/cost than Qdrant at scale.  
- SRP preserved: every capability is now a `CapabilityProvider`; indexing lives in one place.  
- No half-measures. We delete first, then rebuild.

**Total estimated effort**: **38–52 AI-hours** (~260k–380k tokens).  
(Assumes one senior AI-agents Rust developer working at full speed with Claude-4 / Grok-4 level context.)

**Success criteria (must pass before merge)**  
- Single `timescale/timescaledb:18-latest` container powers everything.  
- Zero references to Qdrant, rig-qdrant, or old ToolProvider anywhere.  
- All vector ops go through `rig::VectorStoreIndex` impl from rig-postgres.  
- CocoIndex “codebase-indexer” capability is registered and live-watches real FS.  
- `cargo clippy --all-targets -- -D warnings` + `cargo test` + `cargo nextest` pass cleanly.  
- OpenAPI + /health still work; semantic capability search returns fresh embeddings.

### Phase 0: Preparation & Workspace Reset (2–3 AI-hours)
1. Create branch `refactor/v0.3.1-nuclear-postgres-cocoindex` (no protection).
2. Update root `Cargo.toml`:
   ```toml
   [workspace.package]
   version = "0.3.1"
   edition = "2024"
   rust-version = "1.88"
   ```
3. In `[workspace.dependencies]` (replace entire section):
   ```toml
   rig-core = "0.36"
   rig-postgres = "0.2.5"          # ← canonical Postgres backend
   cocoindex = "1.0"               # Rust engine (incremental + Tree-sitter)
   # remove rig-qdrant, qdrant-client entirely
   # keep fastembed, tokio, axum 0.8, etc. (see v0.3 table)
   ```
4. Run `cargo update && cargo check` to validate.
5. Delete any existing `.env` Qdrant vars; add:
   ```
   DATABASE_URL=postgres://...
   WORKSPACES_ROOT=./workspaces
   ```

### Phase 1: Infrastructure Nuclear Reset (4–5 AI-hours)
1. Delete Qdrant service + volume from `docker-compose.yml`.
2. Replace with single DB service:
   ```yaml
   services:
     postgres:
       image: timescale/timescaledb:18-latest
       environment:
         POSTGRES_DB: conusai
         POSTGRES_USER: conusai
         POSTGRES_PASSWORD: conusai
       volumes:
         - postgres_data:/var/lib/postgresql/data
         - ./workspaces:/app/workspaces:rw
       command: postgres -c shared_preload_libraries=vectorscale
       healthcheck:
         test: ["CMD-SHELL", "pg_isready -U conusai"]
   ```
3. Add init script `docker/init/01-extensions.sql`:
   ```sql
   CREATE EXTENSION IF NOT EXISTS vector;
   CREATE EXTENSION IF NOT EXISTS vectorscale CASCADE;
   ```
4. Update `Makefile` with targets: `db-up`, `db-reset`, `indexer-dev`.

### Phase 2: Crate Structure & Common Layer Cleanup (5–6 AI-hours)
1. Recreate exact v0.3 layout under `apps/backend/` (delete old crates if they conflict).
2. In `crates/common/`:
   - Add `src/db.rs` with `PostgresPool` + connection pooling (sqlx or tokio-postgres).
   - Move `PromptTemplate`, `Ulid`, errors, config (figment) here.
3. Run `cargo fmt && cargo clippy --fix`.

### Phase 3: agent-core – Full CapabilityProvider Rebuild (12–15 AI-hours) ← heaviest phase
1. In `crates/agent-core/src/`:
   - Delete entire old vector/Qdrant module tree.
   - Add `vector_store/postgres.rs` that implements `rig::VectorStoreIndex` using `rig-postgres::PostgresVectorStore`.
   - Create new `capabilities/` directory with:
     - `trait CapabilityProvider`
     - `struct CapabilityFactory`
     - `struct CapabilityCard`
     - `struct CapabilityAdmin`
     - `PromptChainCapability`
2. Implement `CocoIndexCapability` (the star):
   ```rust
   pub struct CocoIndexCapability {
       indexer: cocoindex::IncrementalIndexer,
       vector_store: PostgresVectorStore,
   }

   #[async_trait]
   impl CapabilityProvider for CocoIndexCapability {
       async fn execute(&self, request: CapabilityRequest) -> Result<CapabilityResponse> { ... }
       fn manifest(&self) -> CapabilityCard { ... }
   }
   ```
3. Wire CocoIndex in `AgentBuilder`:
   ```rust
   pub fn with_codebase_indexer(mut self, workspaces_root: PathBuf) -> Self {
       let indexer = cocoindex::IncrementalIndexer::new(workspaces_root, tree_sitter_config, fastembed::TextEmbedding::new());
       let cap = CocoIndexCapability::new(indexer, self.postgres_store.clone());
       self.capability_registry.register("codebase-indexer", cap);
       self
   }
   ```
4. Update `Agent` wrapper to inject the registry.

### Phase 4: Indexing Pipeline & Real FS Integration (6–8 AI-hours)
1. In `crates/agent-core/src/indexing/`:
   - `real_fs_watcher.rs` using inotify + CocoIndex delta-only pipeline.
   - Tree-sitter grammars auto-loaded by CocoIndex for Rust/Python/etc.
   - Embeddings → Postgres sink via rig-postgres.
2. Start indexer automatically on workspace creation (real FS mount).
3. Expose `/v1/capabilities/search` as semantic search over the same Postgres index.

### Phase 5: agent-gateway & API Layer (5–7 AI-hours)
1. Rebuild all three routers (`public_router`, `protected_router`, `admin_router`) using new `CapabilityAdmin` and `CapabilityFactory`.
2. Update handlers:
   - `/v1/chat/completions` and `/v1/agent/completions` now route through `CapabilityProvider` chain.
   - `/v1/capabilities/search` uses Postgres + pgvectorscale DiskANN.
3. Refresh utoipa 5 OpenAPI specs.
4. Remove any old Qdrant client code.

### Phase 6: Polish, Observability & Validation (4–5 AI-hours)
1. Add tracing spans for indexing and retrieval.
2. Update `main.rs` CORS, healthcheck (now reports Postgres + capability count).
3. Delete every remaining reference to old stack (grep for qdrant, ToolProvider, etc. → must return zero).
4. Run full test suite + evals.
5. Update main project instructions document (Key Workspace Dependencies table + Agent-Core Integration Guideline) with new stack.

### Phase 7: Documentation & Final Sign-off (2–3 AI-hours)
1. Add new ADR in `docs/adr/0003-unified-postgres-cocoindex.md`.
2. Update README and start.sh/stop.sh if needed.
3. Tag `v0.3.1` once everything passes.

**Challenge log (why we are not doing X)**  
- No migration scripts → we delete first.  
- No dual-DB mode → single source of truth enforced.  
- No custom VectorStoreIndex wrapper beyond what rig-postgres already provides → reuse is king.  
- CocoIndex is the only indexer → no fallback to naive walk+embed.

This plan is now the authoritative execution path. Once you say **“EXECUTE PHASE 0”** I will output the exact file diffs / code for that phase (ready to copy-paste).  

Ready when you are.