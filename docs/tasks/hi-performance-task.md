**ConusAI Full Clean-Slate Migration Plan: Hetzner CPX52 + RustFS + Qdrant + redb + Marker (Zero SQL, Maximum Performance & Reliability)**

**Executive summary**  
This is a **complete, no-backward-compatibility migration**. We delete every trace of SQL (Postgres, SQLite, sqlx, rig-postgres, pgvector, all migrations).  

**Target architecture (May 2026 gold standard for Rig agent platforms at 100-user / 10k-PDF scale)**  
- **RustFS beta.2** — sole S3-compatible object store (2.3× faster than MinIO on 4 KB objects; our exact workload).  
- **redb v4.1** — embedded, pure-Rust, ACID metadata store for **all** structured data (threads, messages, workspace_nodes + ACLs, audit_events, dynamic_prompts, capability_specs). Zero network, crash-safe, highest performance.  
- **Qdrant 1.x** — single-purpose vector/ANN store (capability + content embeddings + semantic router). Uses official `rig-qdrant`.  
- **Marker API** — PDF → Markdown conversion (CPU; GPU-ready later).  

**Why this wins performance + reliability**  
- redb = embedded → no DB service, sub-millisecond metadata ops, perfect for agent loops and UI tree views.  
- Qdrant = dedicated ANN engine (no SQL mixing).  
- RustFS = fastest small-object S3 for MD/artifacts.  
- 3 Docker services only → simplest, most reliable single-node stack on CPX52 (€25–36/mo). No orchestration theater.  

**Canonical name changes (bold, per style guide + SRP)**  
- All `*Postgres*` / `*Sqlite*` / `*PgVector*` → deleted.  
- New: `RedbMetadataStore` (unifies `ThreadStore` + `WorkspaceStore` + `AuditStore` + dynamic/capability metadata).  
- New: `QdrantVectorStore` (replaces `PgVectorStore`; uses `rig-qdrant`).  
- `MinioWorkspaceContent` → `RustFsContentStore` (still uses `object_store::AmazonS3Builder`).  
- `SemanticCapabilityRouter`, `CapabilitySpecFactory`, `DynamicPromptCapability`, `ArtifactBridge`, `WorkspaceIndexer` updated to call the new stores directly (no pluggability).  

**Effort estimate**  
- **65 AI-hours** total (38 code + 12 infra/config + 10 migration/testing + 5 polish).  
- **≈ $0.55 token cost** (targeted, surgical edits only).  

### Phase 0: Infrastructure (Hetzner + Docker) — 6 AI-hours

1. Provision **Hetzner Cloud CPX52** (8 vCPU / 32 GB / 160 GB NVMe).  
2. Replace `docker-compose.yml` with this exact minimal 2026 starter (copy-paste):

```yaml
version: "3.9"
services:
  rustfs:
    image: rustfs/rustfs:latest   # beta.2
    ports: ["9000:9000", "9001:9001"]
    volumes: ["rustfs_data:/data"]
    environment:
      - RUSTFS_ACCESS_KEY=conusai
      - RUSTFS_SECRET_KEY=conusai-secret-2026
      - RUSTFS_CONSOLE_ENABLE=true
      - RUSTFS_BUCKET=workspace
    restart: unless-stopped
    healthcheck: { test: ["CMD", "curl", "-f", "http://localhost:9000/minio/health/live"], interval: 10s }

  qdrant:
    image: qdrant/qdrant:latest
    ports: ["6333:6333", "6334:6334"]
    volumes: ["qdrant_data:/qdrant/storage"]
    restart: unless-stopped

  marker-api:
    image: savatar101/marker-api:latest
    ports: ["8000:8000"]
    restart: unless-stopped

  rig-agent:
    build: .
    ports: ["8080:8080"]
    depends_on:
      rustfs: { condition: service_healthy }
      qdrant: { condition: service_started }
      marker-api: { condition: service_started }
    environment:
      - CONUSAI_STORAGE_BACKEND=rustfs
      - QDRANT_URL=http://qdrant:6334
      - S3_ENDPOINT=http://rustfs:9000
      - S3_BUCKET=workspace
      - AWS_ACCESS_KEY_ID=conusai
      - AWS_SECRET_ACCESS_KEY=conusai-secret-2026
      - MARKER_URL=http://marker-api:8000
      - ANTHROPIC_API_KEY=sk-...
      - JWT_SECRET=...
      - CONUSAI_WORKSPACE_ROOT=/workspaces
      - RUST_LOG=info
    volumes:
      - ./workspaces:/workspaces
      - redb_data:/data/redb   # redb file lives here
    restart: unless-stopped

volumes:
  rustfs_data:
  qdrant_data:
  redb_data:
```

Run: `docker compose up -d --build`.

### Phase 1: Cargo Workspace & Config Cleanup — 8 AI-hours

- **crates/common/Cargo.toml** & **crates/agent-core/Cargo.toml**:  
  - Remove: `sqlx`, `rig-postgres`, `postgres` feature flags, all Postgres-related deps.  
  - Add:  
    ```toml
    redb = "4.1"
    qdrant-client = "1"
    rig-qdrant = "0.2"
    ```
- **common::config**: Remove all SQL fields. Add:
  ```rust
  pub struct StorageConfig {
      pub backend: StorageBackend, // only RustFs variant now
      pub qdrant_url: String,
  }
  ```
- **AppState::from_env()** in `agent-gateway`: always construct `RedbMetadataStore` + `QdrantVectorStore` + `RustFsContentStore`. No conditionals.

### Phase 2: New Stores (Core Refactor) — 20 AI-hours

**Location:** `crates/agent-core/src/store/`

1. `redb_metadata.rs` — `RedbMetadataStore` (new canonical name).  
   Implements the **existing** `ThreadStore`, `WorkspaceStore`, `AuditStore` traits (plus dynamic prompts & capability specs) using redb tables:
   - `threads`, `messages`, `workspace_nodes`, `audit_events`, `dynamic_prompts`, `capability_specs`.
   - Use typed `TableDefinition`, multi-table transactions, indexes on `tenant_id`, `parent_id`, `name`, etc.

2. `qdrant_vector.rs` — `QdrantVectorStore`.  
   Uses `rig-qdrant` for:
   - `capability_embeddings` collection
   - `content_embeddings` collection  
   - Full `SemanticCapabilityRouter` + `WorkspaceIndexer` integration.

3. `rustfs_content.rs` — `RustFsContentStore` (renamed from MinioWorkspaceContent).  
   `object_store::AmazonS3Builder` pointed at RustFS (zero functional change).

Update:
- `memory/mod.rs` → re-export only the new stores.
- `indexing/coco_indexer.rs`, `vector_store/mod.rs`, `realtime/mod.rs`, `bridge/artifact_bridge.rs` → inject `RedbMetadataStore` + `QdrantVectorStore`.
- `capabilities/providers/*` (DynamicPrompt, CapabilitySpecFactory) → use `RedbMetadataStore`.

Delete entire `memory/postgres_*` and `vector_store/postgres.rs`.

### Phase 3: Capability & Indexing Layer Updates — 12 AI-hours

- `CapabilityRegistry::with_all_factories` → hardcoded for new stores.
- `SemanticCapabilityRouter` → uses `QdrantVectorStore` (already rig-qdrant ready).
- `WorkspaceIndexer` → for PDFs: call Marker API → write MD to RustFS → embed via Qdrant.
- `ArtifactBridge` → unchanged (uses `RustFsContentStore`).
- Remove all Postgres migration files and `capability_specs_changed` LISTEN logic (redb has no need).

### Phase 4: One-Time Data Migration & Cutover — 10 AI-hours

1. Stop old stack.  
2. Export old Postgres → JSONL (simple `sqlx` one-off binary or pg_dump + jq).  
3. Run migration binary (`cargo run --bin migrate-to-redb-qdrant`):  
   - Load JSONL into `RedbMetadataStore`.  
   - Re-embed all capability + content data into Qdrant via `QdrantVectorStore`.  
   - Copy MinIO bucket → RustFS via `mc` / `rclone` (S3 compatible).  
4. `docker compose up -d` new stack.  
5. Run `admin/capabilities/reload` + full indexer pass.

### Phase 5: Verification & Polish — 9 AI-hours

- Update all tests (`CONUSAI_TEST_MODE` now uses in-memory redb + Qdrant noop).  
- Update `docs/verify/verify.md` and `docs/arch.md`.  
- Add OTel metrics: `storage.redb`, `vector.qdrant`, `storage.rustfs`.  
- Run full e2e: upload → Marker → index → semantic search → agent loop → realtime WS → browser-shell.  
- Load test 100 concurrent users.

**Post-migration state**  
- Zero SQL code.  
- Single binary + 3 Docker services.  
- Sub-100 ms agent turns, fastest possible small-object I/O, crash-safe metadata.  
- Future scaling path: still single-node until you genuinely need more (then add Qdrant cluster or dedicated redb replica — still no K8s).

This plan follows every rule: SRP, clean canonical names, newest idiomatic practices (redb + rig-qdrant + RustFS), minimal diff, no unnecessary features.  

Ready for the exact diff patches / new files? Say the word and I’ll generate them file-by-file.