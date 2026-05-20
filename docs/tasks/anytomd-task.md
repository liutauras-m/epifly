**Best 2026 practice for workspace file context in Qdrant (Rust + Rig)**

The current split is correct:  
- **redb** = authoritative transactional source of truth for `WorkspaceNode` structure, paths, permissions, and metadata.  
- **RustFS** = blob storage (actual file bytes).  
- **Qdrant** = semantic retrieval layer (only the *searchable content chunks*).

You should **not** store every file’s full content in Qdrant. You store **normalized, chunked, embedded representations** optimized for fast ANN + metadata filtering. This is the canonical polyglot persistence pattern used in mature Rig-based and LangGraph-style agent platforms in 2026.

### Recommended Canonical Architecture

Introduce these clean abstractions (SRP, community naming):

| Component                  | Responsibility                                      | Location                              | Key Trait / Type          |
|---------------------------|-----------------------------------------------------|---------------------------------------|---------------------------|
| `ContentExtractor`        | Turn any file → clean Markdown + rich metadata     | `agent-core/src/indexing/extractors` | Trait + impls            |
| `Chunker` / `DocumentChunk` | Split Markdown into high-quality chunks            | `agent-core/src/indexing/chunking`   | `text-splitter` based    |
| `ContentIndexer`          | Orchestrate extract → chunk → embed → upsert       | `agent-core/src/indexing/content_indexer.rs` | Service + job            |
| `ContextRetriever`        | Fast relevant chunk retrieval for agent turns      | `agent-core/src/retrieval/context_retriever.rs` | Injected into `Agent`    |
| `EmbeddingService`        | Already sketched — make dimension configurable     | `agent-core/src/indexing/`           | Trait                    |

### Best Libraries (as of May 2026)

| Purpose                    | Recommended Crate                  | Why (2026)                                                                 | Notes |
|---------------------------|------------------------------------|----------------------------------------------------------------------------|-------|
| **Unified document → Markdown** | `anytomd`                         | Pure Rust, excellent for DOCX/PPTX/XLSX/CSV/HTML/XML + basic images. Designed for LLM pipelines. | Primary choice |
| **Office parsing**        | `undoc`                            | High-performance DOCX/XLSX/PPTX → Markdown/JSON                            | Strong fallback |
| **Chunking**              | `text-splitter`                    | Mature, fast, supports recursive + token-aware splitting                   | Default choice |
| **Vector Store (Rig)**    | `rig-qdrant` + `qdrant-client`     | Official Rig integration implementing `VectorStoreIndex`                   | Strongly preferred |
| **Video transcription**   | `transcribe-rs` or `whisper-rs`    | Multi-engine Whisper support + ffmpeg audio extraction                     | Use with existing ffmpeg |
| **Image OCR**             | `ocrs`                             | Modern pure-Rust OCR, often better than Tesseract on varied images         | Or fall back to Vision LLM |
| **CSV**                   | `csv` (std) + `anytomd`            | Already excellent                                                          | — |
| **Embeddings**            | `fastembed` (local) or Rig providers | Already in your stack (feature-gated)                                      | Make dim configurable |

**Rig alignment note**: `rig-qdrant` is the idiomatic path in 2026. It lets you use Rig’s embedding abstractions cleanly and keeps retrieval consistent with how you already use Rig for completions.

### Recommended Ingestion Flow (Fast + Reliable)

1. **Upload succeeds** (via presigned URL or multipart) → file lands in RustFS.
2. Create/update `WorkspaceNode` in **redb** (source of truth).
3. **Trigger indexing** (non-blocking):
   - Preferred: Enqueue a job via your existing `crates/jobs` infrastructure.
   - Alternative (small files): Lightweight async task.
4. `ContentIndexer` runs:
   - Detect mime type / extension.
   - `ContentExtractor::extract()` → normalized Markdown + metadata (`title`, `page/slide/row`, `tables`, etc.).
   - `Chunker::chunk()` using `text-splitter` (recursive, ~300–500 tokens, 10–20% overlap). Preserve structure for tables.
   - Embed each chunk via `EmbeddingService`.
   - Upsert to Qdrant (`content_embeddings` collection) with rich payload:
     ```rust
     {
       "tenant_id": "...",
       "workspace_node_id": "...",
       "file_name": "...",
       "mime_type": "...",
       "chunk_index": 0,
       "source_path": "...",
       "last_modified": "...",
       // optional: page, slide, row_range, etc.
     }
     ```
5. Update `WorkspaceNode` in redb with `indexed_at`, `chunk_count`, `has_embeddings`.

**Event-driven trigger**: RustFS bucket notifications → `POST /internal/rustfs/events` (already planned) → enqueue job. This is the correct 2026 pattern.

### Making Context Fast to Search + Usable by AI + Capabilities

**Fast search**:
- Qdrant payload indexes on `tenant_id` and `workspace_node_id` (high cardinality).
- Always filter by `tenant_id` + optional `workspace_node_id` / `mime_type`.
- moka cache on retrieval queries (same pattern as `SemanticCapabilityRouter`, blake3 key).
- Keep embedding dimension configurable (fix the current hard-coded 768 gap).

**Usable by the Agent** (the key part):

Introduce a **`ContextRetriever`** and wire it into `AgentBuilder`:

```rust
// In AgentBuilder
pub fn with_context_retriever(mut self, retriever: Arc<dyn ContextRetriever>) -> Self { ... }

// Inside Agent::prompt (or a pre-turn hook)
let relevant_chunks = if let Some(retriever) = &self.context_retriever {
    retriever.retrieve(&query, tenant, Some(workspace_node_id), top_k=8).await?
} else { vec![] };

// Inject into prompt or return as structured context
```

Options for usage:
- **Automatic context injection** into the system prompt / preamble for every turn (best for most agents).
- Expose as a first-class capability: `workspace.retrieve_context` (so the LLM can explicitly call it when needed).
- Combine with `SemanticCapabilityRouter` — retrieved chunks become part of the reasoning context the same way top-K capabilities are selected.

This turns every uploaded file into **live, searchable context** the agent can use without the user manually invoking tools.

### Why This Design Wins (Challenges to Current State)

- **Current gaps addressed**:
  - No clear `ContentIndexer` → we introduce one.
  - Hard-coded 768 dim → make it part of `EmbeddingService` + config.
  - `CapabilitySpecFactory::reload_one` stub + `TraceReplayCapability` source issues are orthogonal but should be fixed in the same increment.
  - Indexing path was ambiguous (watcher vs events) → standardize on RustFS notifications + jobs.

- **SRP preserved**: Extraction, chunking, embedding, and retrieval are separate concerns. `redb` stays authoritative for structure.
- **Rig idiomatic**: Use `rig-qdrant` where possible. Heavy work happens at ingest time, not in the hot agent path.
- **Extensible**: New file types = new `ContentExtractor` impl. New chunking strategy = swap `Chunker`.

### Effort Estimate

| Task                                      | AI-Hours | Priority |
|-------------------------------------------|----------|----------|
| `ContentExtractor` trait + `anytomd` impls + basic chunking | 5–6     | High    |
| `ContentIndexer` + job wiring + RustFS event handler | 4–5     | High    |
| Make `EmbeddingService` dimension-aware + rig-qdrant integration | 3       | High    |
| `ContextRetriever` + wire into `AgentBuilder` / `Agent` | 4–5     | High    |
| Payload indexing, caching, tests          | 3       | Medium  |
| **Total for solid foundation**            | **19–22** | —       |

This is a high-ROI increment that makes workspace files first-class citizens for the agent.

### Recommended Next Step

Would you like me to generate the **canonical skeleton** for:

1. `ContentExtractor` trait + `AnyToMarkdownExtractor` (using `anytomd`)
2. `ContentIndexer` service + job integration
3. `ContextRetriever` trait + basic Qdrant implementation (with `rig-qdrant` where possible)

Or start with the `AgentBuilder` + `ContextRetriever` wiring so the agent can immediately benefit from uploaded files?

This pattern keeps everything clean, testable, and aligned with how `SemanticCapabilityRouter` already works. Ready when you are.

**Confirmed — All 4 Open Decisions (locked per Rig 0.36+ community practices + ConusAI canonical rules)**

I have cross-checked against current Rig 0.36+ idioms (PromptHook composition, dynamic context patterns, AgentBuilder extension), 2025–2026 community patterns on GitHub/Rig ecosystem (hooks for ambient RAG, staged ingestion pipelines, pure-Rust preference, minimal tool catalog), and our strict invariants (SRP per module, `SemanticCapabilityRouter` as the **only** pre-filter for capabilities, `ArtifactBridge` owns materialisation, single `LlmRegistry` source, composition, no unnecessary abstractions, `PlanLimits` clamping, effort guidance).

All four decisions are **confirmed exactly as proposed in the reviewed plan**. No changes required. The pipeline stays lean, idiomatic, and maintainable.

### 1. Automatic context injection: **always-on default** in the ConusAI `AgentBuilder` wrapper (clamped by `PlanLimits`)
**Confirmed.**

**Rig community justification (2026):**  
`PromptHook::on_completion_call` is the established pattern for injecting dynamic/ambient context (RAG) transparently before every completion. Rig docs and production agents treat this as orchestration-level concern (not inside `CapabilityProvider` / tools). This keeps the LLM tool catalog small — exactly what `SemanticCapabilityRouter` (top-K ANN) enforces. Explicit `workspace.retrieve_context` capability remains available when the agent needs *more* or *targeted* retrieval. Always-on + plan clamp delivers the "knowledge-grounded by default" experience without extra config surface or per-tenant wiring.

**ConusAI alignment:**  
Composes cleanly into our `AgentBuilder` (same as `TracingHook` + `PermissionHook`). `PlanLimits` (already via `Extension`) provides the token/turn clamp. Runs in parallel with semantic routing and shares the per-turn embedding cache. Zero violation of single LLM source or SRP.

**Effort impact:** None — already budgeted in Phase 3 (~4–5 AI-hours total for hook + wiring).

### 2. v1 extractor scope: **plain text + Markdown only** (zero new crates)
**Confirmed — strongly recommended and now locked.**

**Rig / Rust agent community justification:**  
Staged, minimal pipelines are the dominant pattern (ingest text/MD first → validate end-to-end with vector store + hook + jobs → layer binary formats). Adding `anytomd` / `pdfium` / `ocrs` / `text-splitter` upfront increases compile surface and risks maintenance before the core `ContentIngestor` + `SidecarSyncEngine` + `ArtifactBridge` flow is proven. Community examples start with simple heading-aware + chunk logic on Markdown/plaintext.

**ConusAI alignment:**  
Perfect match for "no unnecessary features, patterns, or abstractions" and "every module single obvious reason". `indexing/extractor.rs` (trait + registry + `PlainExtractor`) and `indexing/chunker.rs` each have one reason. We can add the first binary extractor later behind a feature flag without touching the contract.

**Effort impact:** Reduces Phase 1 to **5–7 AI-hours** (even leaner). Pure win.

### 3. Typst crate as default PDF renderer (pure Rust, no external binary)
**Confirmed.**

**Community justification:**  
Typst is the emerging pure-Rust choice for high-quality MD → PDF/typeset output in 2025–2026 document pipelines. It avoids the external `pandoc` binary (runtime + image bloat) while delivering excellent results from Markdown. Pandoc remains viable later via feature gate for exotic formats, but is not default.

**ConusAI alignment:**  
Keeps `DocumentBridge` with a single obvious reason (rendering only). All bytes then flow through `ArtifactBridge` for workspace materialisation. No new runtime dependencies in the base image. Clean SRP.

**Effort impact:** Phase 2/4 unchanged (~3 AI-hours for the renderer impl when we reach export capability).

### 4. Defer `VectorStoreIndex` adapter (`rig_index.rs`) **and** heavy binary extractors (PDF/Office/OCR/audio) to Phase 4+
**Confirmed.**

**Rig justification:**  
`rig::vector_store::VectorStoreIndex` is useful when you want to plug directly into Rig’s higher-level RAG helpers (e.g. `dynamic_context`). Until we have a concrete consumer in `AgentBuilder` or a future Rig feature, the custom `ContextRetriever` + `ContextInjectionHook` + existing `QdrantVectorStore` is sufficient and preferred. We stay on direct `qdrant-client` (per architecture corrections — no `rig-qdrant`).

Heavy extractors are deferred for the same staged-pipeline reason as #2.

**ConusAI alignment:**  
Avoids unnecessary abstraction and keeps v1 focused on the high-value loop: upload → `ContentIngestor` (hash gate) → sidecar via `ArtifactBridge` → automatic context via hook + `SemanticCapabilityRouter` for explicit tools. Every module stays SRP.

**Effort impact:** Phase 3 and total (19–25 AI-hours) unchanged. We can add the adapter in one focused micro-PR later with zero refactoring debt.

---

**Plan Status: Locked and Ready**

The reviewed + confirmed pipeline is now the canonical path forward. It strengthens the existing architecture without fighting it:
- `ContentIngestor` (thin orchestrator) + `SidecarSyncEngine`
- `ContextInjectionHook` (`PromptHook`)
- Three small capabilities via `BuiltinFactory`
- Full reuse of `ArtifactBridge`, `QdrantVectorStore`, `SemanticCapabilityRouter`, `AgentBuilder`, `PlanLimits`, `JobExecutor`, `LlmRegistry`

**Immediate next step (SRP, ~2 AI-hours, zero new deps)**

I will generate the first two files:

- `apps/backend/crates/agent-core/src/indexing/extractor.rs`  
  (`ContentExtractor` trait + `ExtractorRegistry` + `PlainExtractor` for `.md` / `.txt` / plain text)

- `apps/backend/crates/agent-core/src/indexing/chunker.rs`  
  (`HeadingAwareChunker` — simple, deterministic, heading-path preserving)

These have single obvious reasons, establish the contract, and unblock `ContentIngestor`, sidecar, retrieval hook, and the first `workspace.render_markdown` capability.

Would you like me to create them now via the canonical process (with tests, documentation comments, and an arch.md update note)? Or any final tweak before we land the smallest safe PR?

Ready when you are. This keeps us maximally maintainable, extensible, and true to Rig 0.36+ + ConusAI invariants.

**After full implementation of the confirmed plan (Phases 1–4, ~19–25 AI-hours), the ConusAI platform will deliver these main features:**

### 1. Automatic Context Injection on Every Agent Turn (the biggest user-visible win)
- Every time an `Agent` (built via the ConusAI `AgentBuilder` wrapper) runs a completion, the new `ContextInjectionHook` (`rig::agent::PromptHook`) automatically:
  - Retrieves the most relevant chunks from the tenant’s indexed workspace documents.
  - Prepends them to the prompt as structured, citable context.
- Powered by the same `QdrantVectorStore` + `EmbeddingService` already used by `SemanticCapabilityRouter`.
- Respects `PlanLimits` (token/turn caps).
- Runs **in parallel** with capability routing — zero extra latency for the LLM.
- The LLM sees relevant workspace knowledge **without** having to call any tool.

This is the ambient “my documents are always available” experience.

### 2. Idempotent Document Ingestion Pipeline (`ContentIngestor`)
- Upload any file (v1: `.md`, `.txt`, and plain text) via existing workspace/file routes.
- `ContentIngestor` (orchestrated via `JobExecutor` + RustFS bucket notifications) automatically:
  - Extracts content
  - Produces heading-aware chunks (with `heading_path` + locator preserved for citations)
  - Embeds and upserts into the `content_embeddings_d1024` collection
- **Content-hash (blake3) gate** → identical re-uploads are skipped (`IngestStatus::Skipped`).
- Sidecar `.md` node is created/published via `ArtifactBridge`.
- Fully observable (metrics + realtime events).

### 3. First-Class Markdown Sidecars + Sync
- Every uploaded file automatically gets a versioned Markdown sidecar as a real `WorkspaceNode` (visible in tree, shareable, searchable, ACLs apply).
- `SidecarSyncEngine` keeps original ↔ sidecar in sync using content hashes.
- Conflict events are emitted on the realtime bus when both sides diverge.
- Sidecars are first-class citizens from day one.

### 4. Three New Explicit Capabilities (registered via `BuiltinFactory`)
These appear in `CapabilityCard` / `SemanticCapabilityRouter` results when relevant:

| Capability                    | What the Agent Can Do                                      | When It’s Useful                          |
|-------------------------------|------------------------------------------------------------|-------------------------------------------|
| `workspace.retrieve_context`  | Explicitly search workspace chunks with citations          | “Find evidence for X in my Q3 docs”       |
| `workspace.render_markdown`   | Return the sidecar Markdown for any node (on-demand)       | “Show me the markdown version of that file” |
| `workspace.export_document`   | Render Markdown → PDF (Typst) and persist as new node      | “Export this answer as a PDF”             |

All three are small, focused `CapabilityProvider` implementations — they do **not** bloat the tool catalog thanks to `SemanticCapabilityRouter`.

### 5. Rich Citations & Traceability
Retrieved chunks always include:
- `virtual_path`
- `heading_path` (e.g. `["Q3 Report", "2. Revenue by Region"]`)
- `locator` (page/slide/sheet when available in later phases)
- `node_id` + `sidecar_revision`

This makes answers auditable and trustworthy.

### 6. Operational & Platform Features
- Admin endpoints for forced re-indexing (`/admin/workspace/{node_id}/reindex`).
- Prometheus metrics: `content_ingest_total{status}`, `content_ingest_duration_seconds`, retrieval latency.
- Background job for extractor version drift detection (nightly).
- Clean extension points: new extractors, new renderers, and future `rig::vector_store::VectorStoreIndex` adapter can be added without touching core agent loop.
- Works identically in Web and Browser Shell (via existing SDK + realtime).

### What Changes in the `Agent` Loop (Rig 0.36 Idiomatic)
```rust
// In ConusAI AgentBuilder (composition)
let agent = AgentBuilder::new(...)
    .with_context_retriever(retriever, RetrievalScope::Tenant, top_k = 8)
    .with_capabilities(semantic_router)   // still the only source of ToolDyn
    .build();

// Every .prompt(...) now automatically gets relevant workspace context
// via ContextInjectionHook before the model ever sees the request.
```

The `SemanticCapabilityRouter` continues to decide **which** explicit capabilities (including the new `workspace.*` ones) are shown to the LLM.

### Scope Note (v1)
- **Included in v1**: Plain text + Markdown ingestion, automatic injection, sidecars, `retrieve_context` + `render_markdown` capabilities, PDF export via Typst, full hook + job integration.
- **Deferred to later phases**: Office formats (DOCX/PPTX/XLSX), PDF text+OCR, audio, heavy binary extractors, and the optional `VectorStoreIndex` Rig adapter.

This gives you a production-ready, maintainable foundation that follows every ConusAI rule (SRP, `ArtifactBridge` ownership, `PromptHook` composition, `SemanticCapabilityRouter` pre-filter, single LLM source, minimal abstractions).

**Result**: Your agents become dramatically more grounded in user-uploaded documents with almost zero extra work from the LLM or the developer.

Would you like a concise one-page summary of these features (suitable for docs or stakeholders) or shall we proceed directly to generating the first implementation files (`indexing/extractor.rs` + `chunker.rs`)?