# Workspace Document Context Pipeline — Implementation Plan (v1 locked)

> Companion task: [anytomd-task.md](tasks/anytomd-task.md) — all 4 open decisions locked 2026-05-20.
> Architecture baseline: [arch.md](arch.md) (audit 2026-05-19)
> Plan revision: **v2** (2026-05-20) — supersedes prior plan after task lock-in.

---

## 0. Mission

**Make every uploaded workspace file usable as live context for the agent, with full versioning and end-to-end tracking.**

Three concrete promises:
1. **Ambient context with citations** — every `Agent::prompt` automatically grounds itself in the tenant's relevant workspace content **and the model is contractually told to cite `filename → "heading"` + exact quote**; the LLM does not have to call a tool.
2. **Versioning** — every ingest produces an immutable, hash-pinned revision; nothing is silently overwritten.
3. **Full tracking** — every content version is traceable: who uploaded, when, by which extractor version, how many chunks, what sidecar revision, which embedding-model dims, what the agent retrieved on each turn.

Reference user scenario this plan must satisfy end-to-end: [FR-DOC-001](tasks/functional-task.md) — upload 4 MD docs, ask a question spanning all of them, receive an answer with `filename → heading` citations and exact quotes, with no manual capability call.

---

## 1. v1 Scope (locked)

| Decision | Locked outcome | Source |
|---|---|---|
| Ambient injection | **Always-on** `ContextInjectionHook : rig::agent::PromptHook<M>` in the `AgentBuilder` wrapper, clamped by `PlanLimits` | Task §1 |
| Extractors in v1 | **Plain text + Markdown only** — `.md`, `.txt`, `.json`, `.yaml`, `.toml`, `.csv` (as plain), and any `text/*` mime. No new crates. | Task §2 |
| PDF renderer | **Typst** (pure Rust). Pandoc behind future `--features pandoc`. | Task §3 |
| Rig `VectorStoreIndex` adapter | **Deferred** to a later micro-PR. Custom `ContextRetriever` is sufficient. | Task §4 |
| Heavy extractors (DOCX/PPTX/XLSX/PDF/OCR/audio) | **Deferred** to Phase 5+. Trait + registry land in v1 so they plug in later with zero refactor. | Task §2 + §4 |
| Naming | `ContentIngestor` (not `DocumentPipeline`/`ContentIndexer`) | Task §"Plan Status" |
| Vector store driver | **Existing direct `qdrant-client`** — no `rig-qdrant` in v1 | Task §4 |

Total v1 effort: **19–25 AI-hours** across 4 phases (down from 23–29 in the previous draft).

---

## 2. Current State (verified 2026-05-20)

| Concern | Location | Status |
|---|---|---|
| `EmbeddingService` trait + local fastembed | [embedding_service.rs](apps/backend/crates/agent-core/src/indexing/embedding_service.rs) | ✅ already dim-aware (5 models, `dims()`). The "hard-coded 768" claim in the original brief is stale. |
| `QdrantVectorStore` content collection | [qdrant_vector.rs](apps/backend/crates/agent-core/src/store/qdrant_vector.rs) | ✅ `content_embeddings_dN` exists with payload indexes on `tenant_id` (`is_tenant=true`), `owner_id`, `shared_with`; collection self-heals on dim mismatch. **Nothing populates it today.** |
| `WorkspaceNode` | [common/src/memory/workspace.rs](apps/backend/crates/common/src/memory/workspace.rs) | ✅ untyped `metadata: serde_json::Value`. We will define a typed shape inside it (additive). |
| `/v1/workspaces/search?mode=semantic` | [workspaces.rs](apps/backend/crates/agent-gateway/src/routes/workspaces.rs) | ⚠ route exists, but returns empty (nothing indexes). |
| `/internal/rustfs/events` | gateway internal | ✅ wired; currently no-op for content. |
| `JobExecutor` | [jobs/src/executor.rs](apps/backend/crates/jobs/src/executor.rs) | ✅ `enqueue(name, json) -> Uuid`. SSE per task. |
| `ArtifactBridge` | [bridge/artifact_bridge.rs](apps/backend/crates/agent-core/src/bridge/artifact_bridge.rs) | ✅ persists artefacts → `WorkspaceNode` + emits realtime events. **Reused as the sole sidecar publisher.** |
| `SemanticCapabilityRouter` | [capabilities/semantic_router.rs](apps/backend/crates/agent-core/src/capabilities/semantic_router.rs) | ✅ blake3-keyed moka cache (4096, 60 s). Pattern reused for the retriever. |
| `AgentBuilder` | [agent/builder.rs](apps/backend/crates/agent-core/src/agent/builder.rs) | ✅ composes `TracingHook` + `PermissionHook`. We add one more hook. |
| `audit_events` table in redb | [redb_metadata.rs](apps/backend/crates/agent-core/src/store/redb_metadata.rs) | ✅ existing append-only log. Reused as the **versioning + tracking ledger** (no new tables). |

### Gaps closed by this plan

1. **Content extraction** — none today → `ContentExtractor` trait + `PlainExtractor` + `MarkdownExtractor`.
2. **Indexing orchestration** — `ContentIngestor` ties extract → chunk → embed → upsert.
3. **Versioning** — `WorkspaceNode.metadata` gains a typed `ContentIndexState` block; every revision is logged as an `AuditEvent`.
4. **Ambient retrieval** — `ContextRetriever` + `ContextInjectionHook` wired through `AgentBuilder`.
5. **Sidecar tracking** — `SidecarSyncEngine` keeps `.md` ↔ original in sync via blake3 hashes.
6. **Three explicit capabilities** — `workspace.retrieve_context`, `workspace.render_markdown`, `workspace.export_document` via existing `BuiltinFactory`.

---

## 3. Architecture (lean, SRP, no new crates)

```
agent-core/src/
├── indexing/
│   ├── embedding_service.rs         # existing — dim-aware
│   ├── local_embedding_service.rs   # existing
│   ├── extractor.rs                 # NEW — ContentExtractor trait + ExtractorRegistry
│   ├── extractors/
│   │   ├── plain.rs                 # NEW — text/plain, text/csv, json/yaml/toml as text
│   │   └── markdown.rs              # NEW — MD passthrough + heading harvest
│   ├── chunker.rs                   # NEW — HeadingAwareChunker (deterministic, dependency-free)
│   ├── content_ingestor.rs          # NEW — thin orchestrator (idempotent via blake3)
│   ├── sidecar_sync.rs              # NEW — SidecarSyncEngine state machine
│   └── content_state.rs             # NEW — typed ContentIndexState serde block on metadata
├── retrieval/
│   ├── mod.rs                       # NEW
│   ├── context_retriever.rs         # NEW — trait + QdrantContextRetriever
│   └── injection_hook.rs            # NEW — ContextInjectionHook: PromptHook<M>
├── bridge/
│   ├── artifact_bridge.rs           # existing — publishes sidecar nodes
│   └── document_bridge.rs           # NEW (Phase 2) — DocumentRenderer trait + TypstRenderer
├── capabilities/builtin/
│   ├── workspace_retrieve.rs        # NEW — workspace.retrieve_context
│   ├── workspace_render.rs          # NEW — workspace.render_markdown
│   └── workspace_export.rs          # NEW (Phase 2) — workspace.export_document
└── store/
    └── qdrant_vector.rs             # existing — add upsert_content_chunks + search_content + delete_by_node

apps/backend/crates/jobs/src/jobs/
└── reindex_workspace_node.rs        # NEW — BackgroundJob; calls ContentIngestor

apps/backend/crates/agent-gateway/src/routes/
└── internal_rustfs.rs               # existing — extend to enqueue reindex on ObjectCreated
```

Every new file has exactly one reason to change. No new crates. No `rig-qdrant`. No `anytomd`/`pdfium`/`ocrs` in v1.

---

## 4. Core Contracts

### 4.1 `ContentExtractor`

```rust
#[async_trait]
pub trait ContentExtractor: Send + Sync + 'static {
    /// e.g. ["text/plain", "text/csv"]; matched by ExtractorRegistry::for_mime.
    fn supported_mimes(&self) -> &'static [&'static str];

    /// Stable identifier recorded with each chunk: "plain@1" / "markdown@1".
    /// Bumping this number forces a re-extract via the drift-detection job.
    fn version(&self) -> &'static str;

    /// Pure: bytes + filename + mime → normalised markdown + structured metadata.
    /// MUST NOT touch storage.
    async fn extract(&self, input: ExtractInput<'_>) -> anyhow::Result<ExtractedDocument>;
}

pub struct ExtractInput<'a> {
    pub bytes: &'a [u8],
    pub filename: &'a str,
    pub mime: &'a str,
}

pub struct ExtractedDocument {
    pub markdown: String,
    pub title: Option<String>,
    /// page_count / sheet_names / etc — extractor-defined.
    pub metadata: serde_json::Value,
    pub language: Option<String>,        // best-effort; None in v1
    pub extractor_version: String,       // mirrors trait `version()`
}
```

`ExtractorRegistry::for_mime(mime) -> Option<Arc<dyn ContentExtractor>>` with a last-resort `PlainExtractor` fallback for any `text/*` not otherwise matched. This is the same dispatch pattern as `CapabilityFactory::supports`.

### 4.2 `Chunker`

```rust
pub trait Chunker: Send + Sync + 'static {
    fn version(&self) -> &'static str;   // "heading-aware@1"
    fn chunk(&self, doc: &ExtractedDocument) -> Vec<DocumentChunk>;
}

pub struct DocumentChunk {
    pub index: usize,
    pub text: String,
    pub heading_path: Vec<String>,       // ["Q3 Report", "Revenue"]
    pub char_range: (usize, usize),
    pub locator: Option<serde_json::Value>,  // page/slide/sheet when extractor provides
}
```

v1 default: **`HeadingAwareChunker`** — splits on `#`/`##` boundaries, then a deterministic ~2 000-char window with ~15 % overlap. No external crate; we can swap in `text-splitter` later behind the trait without disturbing callers.

### 4.3 `ContentIngestor` (thin orchestrator)

```rust
pub struct ContentIngestor {
    extractors: ExtractorRegistry,
    chunker: Arc<dyn Chunker>,
    embedder: Arc<dyn EmbeddingService>,
    vector_store: Arc<QdrantVectorStore>,
    workspace_store: Arc<dyn WorkspaceStore>,
    content_store: Arc<dyn WorkspaceContentStore>,
    artifact_bridge: Arc<ArtifactBridge>,
    audit: Arc<dyn AuditSink>,
    realtime: Arc<RealtimeBus>,
}

impl ContentIngestor {
    /// Idempotent. Skips work when ContentIndexState matches current bytes + extractor/chunker/embedder versions.
    pub async fn ingest_node(&self, tenant: &TenantContext, node_id: Ulid, reason: IngestReason)
        -> anyhow::Result<IngestReport>;
}

pub enum IngestReason { Upload, RustFsEvent, AdminReindex, DriftDetected }

pub struct IngestReport {
    pub status: IngestStatus,            // Skipped | Reindexed | Created | Failed
    pub revision: u32,                   // monotonically increases per node
    pub content_hash: [u8; 32],
    pub chunk_count: usize,
    pub sidecar_node_id: Option<Ulid>,
    pub extractor_version: String,
    pub chunker_version: String,
    pub embedder_model: String,
    pub embedder_dims: u64,
    pub elapsed_ms: u128,
}
```

The orchestrator **never** invokes the LLM. All Rig usage stays in `AgentBuilder`/`AgentRuntime`.

### 4.4 `ContextRetriever` + `ContextInjectionHook`

```rust
#[async_trait]
pub trait ContextRetriever: Send + Sync + 'static {
    async fn retrieve(
        &self,
        query: &str,
        tenant: &TenantContext,
        scope: RetrievalScope,
        top_k: usize,
    ) -> anyhow::Result<Vec<RetrievedChunk>>;
}

pub enum RetrievalScope { Tenant, Node(Ulid), PathPrefix(String), Shared }

pub struct RetrievedChunk {
    pub node_id: Ulid,
    pub virtual_path: String,            // full path, used as citation filename
    pub filename: String,                // basename of virtual_path, convenience for citation
    pub heading_path: Vec<String>,       // e.g. ["3. EMEA", "3.2 Performance"]
    pub text: String,                    // raw chunk text, used verbatim in quotes
    pub locator: Option<serde_json::Value>,
    pub distance: f32,
    pub revision: u32,                   // source node revision at retrieval time
    pub sidecar_revision: u32,           // for citation + cache invalidation
}
```

#### 4.4.1 Diversified top-K (FR-DOC-001 requirement)

Multi-document questions fail if one strong document monopolises the result set. `QdrantContextRetriever` therefore applies a **per-node cap** before truncating to `top_k`:

```rust
pub struct RetrievalParams {
    pub top_k: usize,           // default 8
    pub max_per_node: usize,    // default 3 — at most N chunks from any single document
    pub max_distance: f32,      // default 0.45 — drop noise
}
```

Algorithm: oversample (`top_k * 4`) from Qdrant → group by `node_id` → keep the best `max_per_node` per group → flatten back to `top_k` by best distance. Deterministic and cheap; no MMR re-embedding pass needed in v1.

#### 4.4.2 Citation-aware preamble (FR-DOC-001 contract)

The hook formats retrieved chunks into a **structured preamble the model is instructed to cite from**. Format is stable so prompt-engineering and evals can pin against it:

```text
<workspace_context>
You have access to the following excerpts from the user's workspace. When you use any of them, cite them in this exact form:
  `According to **<filename> → "<heading path>"**: "<exact quote>"`
Use only information present in these excerpts and the user's message. Do not invent file names or quotes.

[1] file: Q3_2026_Financial_Report.md
    heading: 3. EMEA Performance > 3.2 Performance
    revision: 4
    excerpt: |
      The €2.4M gap versus plan was almost entirely driven by three delayed enterprise renewals in DACH and France…

[2] file: EMEA_Sales_Deep_Dive.md
    heading: Key Deal Pipeline Risks
    revision: 1
    excerpt: |
      …three deals flagged as at risk due to procurement delays…
</workspace_context>
```

The formatter (`format_chunks_as_context`) lives in `retrieval/injection_hook.rs`, is pure, fully unit-testable, and clamps total tokens to `PlanLimits.context_token_budget` by dropping trailing entries (never truncating mid-quote).

Hook (Rig `PromptHook<M>`):

```rust
impl<M: CompletionModel> PromptHook<M> for ContextInjectionHook {
    async fn on_completion_call(&self, ctx: &CompletionCallContext<'_, M>) -> HookAction {
        let budget = self.plan_limits.context_token_budget();   // PlanLimits clamp
        if budget == 0 { return HookAction::cont(); }
        let chunks = self.retriever
            .retrieve(ctx.prompt_text(), &self.tenant, self.scope.clone(), self.top_k)
            .await
            .unwrap_or_default();
        let block = format_chunks_as_context(&chunks, budget);  // truncates to budget
        if !block.is_empty() {
            ctx.prepend_preamble(block);
            self.realtime.emit("context_injected",
                json!({ "thread_id": self.thread_id, "chunks": chunks_lite(&chunks) }));
        }
        HookAction::cont()
    }
}
```

Wired in `AgentBuilder`:

```rust
AgentBuilder::new(...)
    .with_context_retriever(retriever, RetrievalScope::Tenant, /*top_k*/ 8)
    .with_capabilities(semantic_router)
    .build();
```

The query embedding is computed once per turn and shared with `SemanticCapabilityRouter` via a per-turn `OnceCell` cache.

---

## 5. Versioning & Tracking — the core promise

### 5.1 Typed `ContentIndexState` (lives inside `WorkspaceNode.metadata`)

```rust
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ContentIndexState {
    pub role: SidecarRole,                       // Original | Sidecar | AuthoredMarkdown | Standalone
    pub revision: u32,                           // monotonic; increments on every successful ingest
    pub content_hash: Option<[u8; 32]>,          // blake3 of the file bytes (Original/Authored) or sidecar bytes (Sidecar)
    pub sidecar_node_id: Option<Ulid>,           // set on Original; points to the .md sibling
    pub original_node_id: Option<Ulid>,          // set on Sidecar; back-pointer
    pub sidecar_hash: Option<[u8; 32]>,          // blake3 of the latest sidecar bytes
    pub derived_from_revision: Option<u32>,      // set on Sidecar: which Original revision produced me
    pub extractor_version: Option<String>,       // "plain@1"
    pub chunker_version: Option<String>,         // "heading-aware@1"
    pub embedder_model: Option<String>,          // "multilingual-e5-large"
    pub embedder_dims: Option<u64>,              // 1024
    pub chunk_count: u32,
    pub indexed_at: Option<DateTime<Utc>>,
    pub last_status: Option<String>,             // "ok" | "extract_failed: …"
    pub language: Option<String>,
}
```

Additive: existing nodes deserialize with `..Default::default()` — zero migration.

### 5.2 Storage layout (RustFS)

```
tenant/<tid>/originals/<node_id>/r<rev>/<filename>      # immutable per revision
tenant/<tid>/sidecars/<node_id>/r<rev>/sidecar.md       # immutable per revision
```

Keeping per-revision prefixes (instead of overwriting at a single key) buys us:
- **Point-in-time recovery** — read any past revision.
- **No torn-write races** — readers never see a half-written file.
- **Free rollback** — flip `revision` in `ContentIndexState` to a prior `r<rev>`.

Garbage collection is a separate scheduled job (`prune_old_revisions`, defaults: keep last 5 + everything ≤ 30 days). Out of scope for v1; only the layout decision matters now.

### 5.3 Audit ledger — reuse `audit_events` table

For each ingest we append one `AuditEvent` keyed `(tenant, ts_micros, ulid)`:

```jsonc
{
  "kind": "content.indexed",
  "actor": "system|user:<id>|admin:<id>",
  "tenant_id": "…",
  "node_id": "…",
  "revision": 4,
  "reason": "Upload|RustFsEvent|AdminReindex|DriftDetected",
  "content_hash": "blake3:…",
  "sidecar_node_id": "…",
  "sidecar_revision": 4,
  "extractor_version": "plain@1",
  "chunker_version": "heading-aware@1",
  "embedder_model": "multilingual-e5-large",
  "embedder_dims": 1024,
  "chunk_count": 42,
  "status": "ok",
  "elapsed_ms": 387
}
```

And one `AuditEvent { kind: "content.retrieved", thread_id, query_hash, chunk_refs[], distance_min, distance_max }` per agent turn that injects context.

This satisfies "full tracking" without inventing a new store: it goes through the same `/v1/audit` endpoint admins already use, and is filterable per tenant/node.

### 5.4 Qdrant payload schema (per chunk point)

```jsonc
{
  "tenant_id":         "…",      // payload-indexed, is_tenant=true (existing)
  "owner_id":          "…",      // payload-indexed (existing)
  "shared_with":       ["…"],    // payload-indexed array (existing)
  "node_id":           "…",      // NEW payload index (keyword)
  "virtual_path":      "Clients/Acme/Kickoff.md",
  "chunk_index":       3,
  "heading_path":      ["…","…"],
  "char_range":        [120, 540],
  "locator":           {...},
  "revision":          4,         // NEW — used by delete-by-revision purge
  "extractor_version": "plain@1",
  "chunker_version":   "heading-aware@1",
  "embedder_model":    "multilingual-e5-large",
  "embedder_dims":     1024,
  "content_hash":      "blake3:…",
  "indexed_at":        "2026-05-20T…"
}
```

New payload indexes: `node_id` (keyword), `revision` (integer). Point ID: `uuid5(namespace=node_id, name=format!("{revision}:{chunk_index}"))` — deterministic, idempotent, supports clean delete-by-old-revision.

### 5.5 Idempotency + drift gates inside `ContentIngestor`

```text
let original_bytes = content_store.get(node);
let h = blake3(&original_bytes);
let state = node.metadata.content_index_state;

if state.content_hash == Some(h)
   && state.extractor_version == Some(current_extractor.version())
   && state.chunker_version   == Some(current_chunker.version())
   && state.embedder_model    == Some(current_embedder.model().name())
   && state.embedder_dims     == Some(current_embedder.dims())
{
    return IngestReport { status: Skipped, .. };
}

// else: extract → chunk → embed → upsert new points at revision+1 →
//       delete points where revision < new_revision → publish sidecar via ArtifactBridge →
//       update ContentIndexState → append AuditEvent.
```

A re-upload of identical bytes is **free**. An extractor or embedder version bump triggers a clean re-extract on the next ingest call (or the nightly drift job).

### 5.6 Sidecar sync state machine (`SidecarSyncEngine`)

Inputs: `original_hash_changed`, `sidecar_exists`, `sidecar_hash_matches_derived_from_revision`.

| Original changed | Sidecar exists | Hash matches derived | Action |
|:--:|:--:|:--:|---|
| no | yes | yes | **Skip** |
| no | yes | no | **Conflict** → emit realtime `sidecar_conflict`, audit, no destructive change |
| no | no | — | Create sidecar from original |
| yes | yes | yes | Regenerate sidecar; `revision += 1`; clean old Qdrant points |
| yes | yes | no | **Conflict** → same as above |
| yes | no | — | Create sidecar |

`AuthoredMarkdown` inverts the relationship: the uploaded `.md` **is** the source of truth. The sync engine **short-circuits** for `SidecarRole::AuthoredMarkdown` — no sibling sidecar node is created, no duplication, the node is indexed directly. Any exported PDF/DOCX is materialised by `workspace.export_document` on demand and stored as `derived_from_revision` of the MD node. We never auto-regenerate exports — format choice is the user's.

> FR-DOC-001 path: all four uploaded files are `.md`, so they take the `AuthoredMarkdown` short-circuit — single node per file, single Qdrant point set per file, no sidecar fan-out.

---

## 6. Capabilities (kind = `native`)

| Capability | Tools | Purpose |
|---|---|---|
| `workspace.retrieve_context` | `retrieve_context(query, top_k?, node_id?, path_prefix?)` | Explicit RAG; complements ambient injection. |
| `workspace.render_markdown` | `render_markdown(node_id)` | Returns the sidecar MD; extracts on-the-fly if missing and persists. |
| `workspace.export_document` | `export_document(node_id, target: "pdf")` | Phase 2; MD → PDF via Typst → new `WorkspaceNode`. |

Registered via the existing `BuiltinFactory` (no new factory). Tenant-scoped exactly like every other native capability; `SemanticCapabilityRouter` decides when they surface to the LLM — they never bloat the catalogue.

---

## 7. End-to-end flow

```
        ┌── upload via /v1/files | /v1/workspaces (presign) ──┐
        ▼                                                      │
RustFS originals/<node>/r<n>/file    →  bucket notification → /internal/rustfs/events
        │                                                                │
        │                              JobExecutor::enqueue("reindex_workspace_node", { node_id, reason })
        ▼                                                                ▼
WorkspaceStore.put_node(redb)                                  ContentIngestor::ingest_node
                                                                          │
                                       hash gate? ── Skipped ─────────────┤
                                                                          │
                                       extract → chunk → embed → upsert   │
                                       publish sidecar via ArtifactBridge │
                                       update ContentIndexState           │
                                       append AuditEvent                  │
                                                                          ▼
                                                          realtime: node_indexed { node_id, revision }

──── agent turn ────
POST /v1/agent/completions
   AgentRuntime → AgentBuilder.prompt
   ├── ContextInjectionHook   (parallel, uses shared query embedding)
   │     retrieve top-K → prepend preamble → audit "content.retrieved"
   └── SemanticCapabilityRouter → top-K capabilities
   model.complete → SSE chunks → tool_call → … → done
```

---

## 8. Rig 0.36 alignment

| Rig idiom | This plan |
|---|---|
| `rig::agent::PromptHook<M>` for cross-cutting concerns | new `ContextInjectionHook` — composes with existing `TracingHook` + `PermissionHook` |
| Single `CompletionProvider` source | unchanged — ingestor never instantiates a provider |
| `rig::tool::ToolDyn` for capabilities | three new `BuiltinFactory` capabilities behind `SemanticCapabilityRouter` |
| `rig::embeddings::EmbeddingsBuilder` | NOT used — `EmbeddingService` keeps the e5 prefix discipline already in [local_embedding_service.rs](apps/backend/crates/agent-core/src/indexing/local_embedding_service.rs) |
| `rig::vector_store::VectorStoreIndex` | **Deferred** — easy to add as a single thin adapter when a Rig consumer requires it |
| `AgentBuilder` composition | one new `with_context_retriever(retriever, scope, top_k)` method; identical pattern to `with_hook` |

No global state. No hidden providers. No bypass of `LlmRegistry` / `SemanticCapabilityRouter` / `ArtifactBridge`. No new top-level abstraction beyond what the task already named.

---

## 9. Phased Roadmap

### Phase 1 — Extraction + indexing foundation · 5–7 AI-h

1. `indexing/extractor.rs` — `ContentExtractor` trait + `ExtractInput` + `ExtractedDocument` + `ExtractorRegistry`.
2. `indexing/extractors/plain.rs` + `markdown.rs`.
3. `indexing/chunker.rs` — `HeadingAwareChunker`.
4. `indexing/content_state.rs` — `ContentIndexState` + `SidecarRole` (additive on `WorkspaceNode.metadata`).
5. `store/qdrant_vector.rs` — `upsert_content_chunks` + `search_content` + `delete_chunks_for_node_below_revision`; add `node_id` + `revision` payload indexes.
6. `indexing/content_ingestor.rs` — `ContentIngestor::ingest_node`, idempotent, audit-emitting.
7. `crates/jobs/src/jobs/reindex_workspace_node.rs` — `BackgroundJob`.
8. `/internal/rustfs/events` → enqueue job on `ObjectCreated:*` for `originals/**`.
9. Unit: identical re-upload ⇒ `Skipped`. Integration: upload `.md` ⇒ row in `content_embeddings_d1024`; `ContentIndexState.revision == 1`; one `content.indexed` audit row.

### Phase 2 — Sidecar sync + Markdown export · 4–6 AI-h

1. `indexing/sidecar_sync.rs` — state machine (§5.6).
2. Use `ArtifactBridge` to materialise sidecar as `WorkspaceNode` (`SidecarRole::Sidecar`).
3. `bridge/document_bridge.rs` + `TypstRenderer` for MD → PDF.
4. Capability `workspace.render_markdown` (returns existing sidecar; triggers ingest if absent).
5. Capability `workspace.export_document` (target=`pdf` only in v1).
6. Realtime events: `sidecar_created`, `sidecar_updated`, `sidecar_conflict`.
7. Verify: re-upload changes original ⇒ sidecar revision bumps; editing sidecar manually ⇒ `sidecar_conflict` event + audit, no destructive overwrite.

### Phase 3 — Retrieval wiring (ambient + explicit) · 5–7 AI-h

1. `retrieval/context_retriever.rs` — `ContextRetriever` trait + `QdrantContextRetriever` with moka cache (4096, 60 s, blake3-keyed) mirroring `SemanticCapabilityRouter`; implements diversified top-K (§4.4.1) with `RetrievalParams { top_k, max_per_node, max_distance }`.
2. `retrieval/injection_hook.rs` — `ContextInjectionHook : PromptHook<M>`, `PlanLimits`-clamped; pure `format_chunks_as_context` formatter producing the citation-contract preamble (§4.4.2); emits `context_injected` realtime + `content.retrieved` audit.
3. `AgentBuilder::with_context_retriever(retriever, scope, params)`; wire in `AppState::build` with `max_per_node = 3` default.
4. Per-turn embedding cache shared between hook + `SemanticCapabilityRouter`.
5. Capability `workspace.retrieve_context` (delegates to retriever; same `RetrievalParams`).
6. SSE: extend chat stream with `context_injected { chunks: [{node_id, virtual_path, filename, heading_path, distance, revision}] }`.
7. Verify: tenant with 4 indexed `.md` files (the FR-DOC-001 fixtures), ask the EMEA-shortfall question ⇒ streamed answer contains the exact citation form, cites ≥ 2 distinct files; cross-tenant isolation unit test asserts `tenant_id` filter is mandatory; unit test for `format_chunks_as_context` pins the preamble byte-for-byte.

### Phase 4 — Tracking surface + admin · 5 AI-h

1. `GET /v1/workspaces/{node_id}/index-state` — returns `ContentIndexState` + last 10 audit rows for the node.
2. `GET /v1/workspaces/{node_id}/revisions` — lists all `r<rev>` revisions from RustFS metadata.
3. `POST /admin/workspace/{node_id}/reindex`, `POST /admin/workspace/reindex-all` (rate-limited).
4. Metrics: `content_ingest_total{status,reason}`, `content_ingest_duration_seconds` (SLO: p95 ≤ 8 s/doc so 4-doc batch < 30 s per FR-DOC-001), `context_retrieve_latency_seconds` (SLO: p95 ≤ 250 ms), `context_retrieve_chunks_returned`, `context_retrieve_distinct_nodes` (asserts diversification works).
5. Nightly cron `extractor_drift_scan` — re-enqueues nodes whose stored versions are older than current.
6. Web slice (`packages/ui/src/lib/features/`) — `NodeIndexStatus.svelte` showing revision, hash, chunk count, last ingest time (read-only; conflict-resolution UI deferred).

**Total v1 effort: 19–25 AI-hours.**

### Phase 5+ (Deferred, called out so trait surface stays sufficient)

- `extractors/anytomd.rs` — DOCX/PPTX/XLSX/HTML/XML/EPUB.
- `extractors/pdf.rs` — text-first → `ocrs` → Claude vision (via existing `ContractPipeline`).
- `extractors/audio.rs` — whisper-rs, behind `--features audio-extract`.
- `retrieval/rig_index.rs` — `rig::vector_store::VectorStoreIndex` adapter.
- `PandocRenderer` (feature `pandoc`) for DOCX/ODT/EPUB export targets.
- `prune_old_revisions` GC job.
- Conflict-resolution UI (`SidecarConflict.svelte`).

Each is one file, one trait impl. No core refactor.

---

## 10. Acceptance Criteria (v1)

- Uploading `.md` / `.txt` causes a `content_embeddings_d1024` row count increase within ≤ 5 s; `ContentIndexState { revision: 1, content_hash, chunk_count > 0 }`; exactly one `content.indexed` `AuditEvent`.
- Re-uploading identical bytes ⇒ `IngestStatus::Skipped`; no new Qdrant points; no new audit row (or one row with `status: "skipped"` — pick one and stick to it; recommended: omit).
- Modifying the original bytes ⇒ `revision` increments by 1; old Qdrant points for `revision < new` are deleted; sidecar revision tracks.
- Editing only the sidecar `.md` ⇒ `sidecar_conflict` realtime event + audit row; original untouched; Qdrant untouched.
- Agent turn in a tenant with indexed content ⇒ `context_injected` realtime event with chunk refs; `content.retrieved` audit row with `query_hash` + `chunk_refs`.
- Cross-tenant query from tenant B never returns chunks owned by tenant A — covered by a unit test that asserts every `search_content` call carries the `tenant_id` filter.
- `GET /v1/workspaces/{node_id}/index-state` returns the typed state for a tenant member; 403 for others.
- `workspace.export_document` round-trips `.md` → PDF and creates a new `WorkspaceNode`; PDF opens in standard viewers.
- `PlanLimits.context_token_budget == 0` ⇒ hook skips injection entirely (free-tier guardrail).
- **FR-DOC-001 end-to-end** ([tasks/functional-task.md](tasks/functional-task.md)): uploading the four sample `.md` files and asking the EMEA-shortfall question yields a streamed answer that (a) contains ≥ 1 citation matching the exact form `According to **<filename> → "<heading>"**: "<quote>"`, (b) cites at least two distinct files, (c) the quoted text appears verbatim in the corresponding `RetrievedChunk.text`, (d) one `content.retrieved` audit row references chunks from ≥ 2 distinct `node_id`s, (e) end-to-end ingestion of all four files completes in < 30 s (background job), (f) re-uploading any of the four files returns `IngestStatus::Skipped`.

---

## 11. Risks & Mitigations

| Risk | Mitigation |
|---|---|
| Ambient injection inflates prompts and bills | `PlanLimits.context_token_budget` clamp + `top_k` cap + per-turn audit makes spend observable. |
| Identical query repeated across turns embeds twice | moka cache on retriever (blake3 key) + per-turn `OnceCell` shared with `SemanticCapabilityRouter`. |
| Conflict storm on heavily edited sidecars | State machine is non-destructive; conflicts only surface as events. UI work deferred but never blocks ingestion. |
| Drift after embedder model change | `embedder_dims` mismatch is already handled by `QdrantVectorStore` (drop & recreate). `embedder_model` mismatch in `ContentIndexState` triggers re-ingest via the nightly drift job. |
| RustFS event flood (rename, ACL change) | Job is idempotent via hash gate — events that don't change bytes are O(1). Concurrency cap with `tokio::sync::Semaphore(4)` in the job. |
| Cross-tenant leakage | `tenant_id` payload filter is mandatory in `search_content`; unit test enforces. |
| Plain extractor mis-detecting binary uploaded as `application/octet-stream` | `PlainExtractor::supported_mimes()` is whitelist-only; non-text mimes return `IngestStatus::Failed { reason: "no_extractor_for_mime" }` without writing junk to Qdrant. |
| One document monopolises top-K, citations miss other files (breaks FR-DOC-001) | `RetrievalParams.max_per_node = 3` cap + oversample-then-group in `QdrantContextRetriever` (§4.4.1); `context_retrieve_distinct_nodes` metric trips alert if < 2 on multi-doc workspaces. |
| Model hallucinates filenames or quotes despite preamble | Preamble explicitly forbids invented filenames; eval suite ([apps/backend/evals](apps/backend/evals/)) adds FR-DOC-001 prompt and asserts citation strings are substrings of the injected preamble. |

---

## 12. Documentation Touch-ups

After v1 lands:
- [arch.md](arch.md) §4.2 — add `indexing/` + `retrieval/` subtrees; mention new payload indexes in §4.7.
- [project-instructions.md](project-instructions.md) §5 — add `ContentIngestor`, `ContextInjectionHook`; clarify ambient-injection is plan-clamped.
- New section in [capability-authoring-guide.md](capability-authoring-guide.md) explaining the difference between **infra pipelines** (ingestion) and **capabilities** (LLM-callable tools) using this work as the canonical example.

---

## 13. First PR (≤ 2 AI-h, zero new deps)

Land two files only:
- `apps/backend/crates/agent-core/src/indexing/extractor.rs` — trait + registry + `PlainExtractor`.
- `apps/backend/crates/agent-core/src/indexing/chunker.rs` — `HeadingAwareChunker`.

Includes:
- Documentation comments tying each public item to this plan.
- Unit tests for `ExtractorRegistry::for_mime` fallback + chunker determinism + heading-path correctness.
- One-line update to [arch.md](arch.md) §4.2 file tree.

This unblocks `ContentIngestor`, the sidecar engine, retrieval, and `workspace.render_markdown` in subsequent micro-PRs without any refactor.
