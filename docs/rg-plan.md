# Rig 0.36 Native Migration Plan

> Source inputs: [docs/rig-features.md](docs/tasks/rig-features.md) (strategic mandate) and [docs/arch.md](docs/arch.md) / [docs/arch2.md](docs/arch2.md) (current implementation audit).
> Goal: replace ConusAI's custom vector / memory / modality / provider glue with **first-class Rig 0.36 abstractions** while preserving tenant isolation, semantic prefilter ≤ 50, and the current capability factory + TOML manifest contract.
> Audit anchor date: 2026-05-11 · backend 0.3.1 · `rig-core = 0.36`.

> **Review-incorporation log (2026-05-11):** Formal review against docs.rig.rs 0.36 + 2026 Rust AI-engineering norms approved this plan and recommended six clarifications, all folded in below:
> 1. Explicit `rig-core` `derive` feature for `#[derive(Embed)]` (§A.1).
> 2. Typed `VectorSearchRequest` + `Filter` AST instead of raw SQL predicates (§A.3).
> 3. `[package.metadata.features]` table + `docs/features.md` for IDE/tooling discovery (§A.1).
> 4. `Arc<tokio::sync::Mutex<lancedb::Connection>>` (or `with_cache`) for hot-path reuse (§A.2).
> 5. `#[serde(tag = "kind")]` tagged-union `ChunkPayload` for forward-compatible streaming (§C.3).
> 6. Quarterly Rig 0.36.x compatibility-matrix CI job + post-migration `cargo udeps` / `cargo machete` / "Rig purity audit" (§4, §5, §6).

---

## 0. Guiding Constraints (do not break)

These remain non-negotiable across every phase below — every PR must prove they still hold:

1. **Single LLM source-of-truth** — every model call still routes through `agent_core::llm::LlmRegistry`. We *deepen* it, we do not bypass it.
2. **Semantic prefilter, top-K ≤ 50** — agent turns must never receive the full tool catalog. After migration this is enforced by a Rig `VectorStoreIndex` query rather than the custom moka+blake3 cache, but the cap stays.
3. **Tenant isolation** — every vector / memory query carries `tenant_id` as metadata; no cross-tenant recall is possible at the index layer.
4. **Plan limits** — `tenant.plan.max_tokens / max_turns / default_alias` continue to clamp every Rig agent build.
5. **No hidden network on boot** — `verify_llm_providers` keeps performing alias-resolution only, never an outbound call.
6. **OpenAPI parity** — every new modality route is `utoipa`-annotated and surfaces in the existing Swagger doc.
7. **Shell parity** — anything added on the backend is reachable from `apps/web` *and* `apps/browser-shell` via `@conusai/sdk` + `createChatStream`.
8. **Reduced motion + a11y** — UI deltas (audio waveforms, image previews) honour `prefers-reduced-motion` and the `paper`/`forge` token system.

---

## 1. Current-State Recap (what is being replaced)

| Concern | Current implementation | Files to retire / refactor |
| --- | --- | --- |
| Vector ANN | `QdrantVectorStore` thin client + `SemanticCapabilityRouter` (moka cache, blake3 keys, manual top-K) | [apps/backend/crates/agent-core/src/store/qdrant_vector.rs](apps/backend/crates/agent-core/src/store/qdrant_vector.rs), [semantic_router.rs](apps/backend/crates/agent-core/src/capabilities/semantic_router.rs), [vector_store/mod.rs](apps/backend/crates/agent-core/src/vector_store/mod.rs) |
| Embeddings | Custom `EmbeddingService` trait + `LocalEmbeddingService` (fastembed) + `OpenAiEmbeddingService` | [indexing/embedding_service.rs](apps/backend/crates/agent-core/src/indexing/embedding_service.rs), [local_embedding_service.rs](apps/backend/crates/agent-core/src/indexing/local_embedding_service.rs) |
| Long-term memory | `ContextBuilder` + `OldestFirstTruncator` walking workspace ancestors | [memory/context_builder.rs](apps/backend/crates/agent-core/src/memory/context_builder.rs), [memory/truncator.rs](apps/backend/crates/agent-core/src/memory/truncator.rs) |
| Multimodal | Vision **input** only via `ContractPipeline::UserContent::image_base64` | [chains/contract.rs](apps/backend/crates/agent-core/src/chains/contract.rs) |
| Providers | `AnthropicProvider` only (custom `CompletionProvider` impl) | [llm/providers/anthropic.rs](apps/backend/crates/agent-core/src/llm/providers/anthropic.rs) |
| Cap registry semantic search | `capabilities/embedding.rs` builds capability text vectors fed into Qdrant | [capabilities/embedding.rs](apps/backend/crates/agent-core/src/capabilities/embedding.rs) |

Touchpoints that **must keep working** through every phase: `AgentRuntime::prompt` ([agent/builder.rs](apps/backend/crates/agent-core/src/agent/builder.rs)), `AppState` wiring ([agent-gateway/src/state.rs](apps/backend/crates/agent-gateway/src/state.rs)), and the gateway tests in [tests/remote_mcp_e2e.rs](apps/backend/crates/agent-gateway/tests/remote_mcp_e2e.rs).

---

## 2. Phased Roadmap (ordered by ROI from `rig-features.md`)

### Phase A — Vector Store: migrate to `rig-lancedb` + `VectorStoreIndex`  *(highest ROI)*

**Outcome:** delete `QdrantVectorStore`, the moka cache, and the blake3 key code; capability prefilter becomes a one-line `index.top_n(query, k)` call. LanceDB is the embedded backend (sub-ms ANN, perfect for the Tauri shell); `rig-qdrant` remains as a feature-gated cloud backend.

#### A.1 Workspace deps
- Edit root [Cargo.toml](Cargo.toml) `[workspace.dependencies]`: bump nothing, add
  ```toml
  rig-core    = { version = "=0.36", features = ["derive"] }   # `derive` required for #[derive(Embed)]
  rig-lancedb = "0.36"
  rig-qdrant  = "0.36"   # cloud backend, feature-gated
  lancedb     = "0.21"   # transitively required for index init
  ```
  Pin Rig with `=0.36` and add a `cargo deny` rule to block accidental minor bumps; CI runs a quarterly "Rig compatibility matrix" job against the latest `0.36.x` patch.
- In [agent-core/Cargo.toml](apps/backend/crates/agent-core/Cargo.toml) add features:
  ```toml
  [features]
  default          = ["vector-lancedb"]
  vector-lancedb   = ["dep:rig-lancedb", "dep:lancedb"]
  vector-qdrant    = ["dep:rig-qdrant"]
  local-embeddings = ["dep:fastembed"]

  [package.metadata.features]
  vector-lancedb   = "Embedded LanceDB ANN backend (default; sub-ms top-K)."
  vector-qdrant    = "Cloud Qdrant backend for multi-node deployments."
  local-embeddings = "Bundle fastembed for offline embedding generation."
  ```
  The `[package.metadata.features]` table is the 2026 community convention for documenting Cargo features so `cargo metadata` and IDEs surface them; also mirrored in a new [docs/features.md](docs/features.md).
- Remove the direct `qdrant-client` dep from `agent-core` once Phase A.5 lands; keep it only behind `vector-qdrant`.

#### A.2 New module `agent_core::vector` (replaces `store::qdrant_vector` + `vector_store/mod.rs`)

Create:
```
agent-core/src/vector/
├── mod.rs              # re-exports + `VectorBackend` enum
├── lancedb.rs          # build_capability_index(), build_content_index()
├── qdrant.rs           # cloud backend (feature = "vector-qdrant")
└── schema.rs           # CapabilityRecord / ContentRecord (Embed derive)
```

`schema.rs` uses Rig's `Embed` derive so Rig owns the embedding pipeline:
```rust
use rig::Embed;

#[derive(Embed, serde::Serialize, serde::Deserialize, Clone)]
pub struct CapabilityRecord {
    pub id: String,                 // capability name (e.g. "fs__read_file")
    pub tenant_id: String,          // metadata filter
    pub namespace: String,
    #[embed]
    pub document: String,           // name + description + tags + schema preview
    pub tags: Vec<String>,
}
```

`lancedb.rs` exposes:
```rust
pub async fn build_capability_index(
    conn: Arc<tokio::sync::Mutex<lancedb::Connection>>,
    embedder: impl rig::embeddings::EmbeddingModel + 'static,
    records: Vec<CapabilityRecord>,
) -> anyhow::Result<rig_lancedb::LanceDbVectorIndex<_>>;
```
The `lancedb::Connection` is held inside an `Arc<tokio::sync::Mutex<…>>` (or `LanceDbVectorIndex::with_cache(...)` if available in the installed `rig-lancedb`) so the Tauri shell hot path reuses one open handle instead of reopening the embedded DB per query.

#### A.3 Replace `SemanticCapabilityRouter`

Refactor [semantic_router.rs](apps/backend/crates/agent-core/src/capabilities/semantic_router.rs):

- Drop `moka`, `blake3` cache, manual top-K loop, custom `CachedResult`.
- Hold `Arc<dyn rig::vector_store::VectorStoreIndexDyn>` instead of `Arc<QdrantVectorStore>`.
- `tool_definitions(query, &tenant)` becomes a typed `VectorSearchRequest` (portable across LanceDB / Qdrant — avoids backend-specific SQL strings):
  ```rust
  use rig::vector_store::request::{VectorSearchRequest, Filter};

  let req = VectorSearchRequest::builder()
      .query(query)
      .samples(self.cfg.top_k.min(50) as u64)
      .filter(Filter::eq("tenant_id", tenant.tenant_id.as_str())
          .and(Filter::in_("namespace", self.cfg.namespace.as_slice())))
      .build();
  let hits = self.index.top_n_ids(req).await?;
  // hits is Vec<(score, id)>; resolve back to CapabilityProvider via registry
  ```
- Tenant + namespace filters are pushed into the Rig `Filter` AST — replaces the hand-rolled `NamespaceFilter` and remains portable when swapping `vector-lancedb` ↔ `vector-qdrant`.
- Keep `include_always` semantics by union-ing those names after the Rig query.
- Preserve the `RouterMetrics` counters (cache_hits/misses become hit/miss against Rig's own caching layer when present, else dropped — wire OTel counters in [common::metrics](apps/backend/crates/common/src/metrics.rs) instead).

#### A.4 Capability indexing pipeline
- Replace [capabilities/embedding.rs](apps/backend/crates/agent-core/src/capabilities/embedding.rs) (manual `embed_capability_text`) with Rig's `EmbeddingsBuilder`:
  ```rust
  let docs = registry.iter_cards()
      .map(CapabilityRecord::from_card)
      .collect::<Vec<_>>();
  let embeddings = rig::embeddings::EmbeddingsBuilder::new(embedder.clone())
      .documents(docs)?
      .build()
      .await?;
  index.insert_documents(embeddings).await?;
  ```
- Move this into `agent-gateway::capabilities::bootstrap` so it runs once during `AppState` build, after capability discovery.

#### A.5 AppState rewire
- In [agent-gateway/src/state.rs](apps/backend/crates/agent-gateway/src/state.rs), replace the `QdrantVectorStore` field with `Arc<dyn VectorStoreIndexDyn>` (capability index) and a separate content index for `coco_indexer`.
- Update the `tests/remote_mcp_e2e.rs` `ConstEmbedder` test double to implement Rig's `EmbeddingModel` trait instead of the custom `EmbeddingService`. This is the canary test for Phase A.

#### A.6 Cleanup
Delete (after grep confirms no remaining references):
- `store/qdrant_vector.rs`
- `vector_store/mod.rs`
- `capabilities/embedding.rs`
- The custom `EmbeddingService` trait family in `indexing/embedding_service.rs` (kept temporarily as a thin newtype around `rig::embeddings::EmbeddingModel` if the `coco_indexer` still needs it; remove fully in Phase A.7).

#### A.7 Acceptance
- `cargo test -p agent-core -p agent-gateway` green.
- New benchmark `benches/router_topk.rs` shows P50 ≤ 2 ms for top-K=20 on 1k capability cards (LanceDB embedded).
- Verified: tenant-A cannot see tenant-B capabilities even when querying with identical text (filter pushed down).
- `RouterMetrics` OTel counters surface in `/metrics`.

---

### Phase B — Provider Breadth: full Rig `ProviderClient` rollout

**Outcome:** `LlmRegistry` resolves any of Rig's 20+ providers from a TOML alias. Custom `CompletionProvider` boilerplate drops to a single ~40-line generic adapter.

#### B.1 Generic provider adapter
Replace per-provider files in `llm/providers/` with **one** generic shim:

```
agent-core/src/llm/providers/
├── mod.rs                 # provider factory map
└── rig_adapter.rs         # impl<C: rig::client::CompletionClient> CompletionProvider for RigAdapter<C>
```

`rig_adapter.rs`:
```rust
pub struct RigAdapter<M: rig::completion::CompletionModel> {
    model: M,
}

#[async_trait::async_trait]
impl<M> CompletionProvider for RigAdapter<M>
where M: rig::completion::CompletionModel + Send + Sync + 'static
{
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError> { … }
    async fn stream(&self, req: LlmRequest) -> Result<LlmStream, LlmError> { … }
}
```

This deletes the bespoke streaming loop in [anthropic.rs](apps/backend/crates/agent-core/src/llm/providers/anthropic.rs). Streaming logic now lives once in the adapter using `rig::streaming::StreamedAssistantContent`.

#### B.2 Feature-flagged provider registration

In `agent-core/Cargo.toml`:
```toml
[features]
provider-anthropic  = []   # always on
provider-openai     = []
provider-gemini     = []
provider-groq       = []
provider-mistral    = []
provider-xai        = []
provider-ollama     = []
provider-azure      = []
provider-perplexity = []
provider-together   = []
all-providers = ["provider-openai","provider-gemini","provider-groq",
                 "provider-mistral","provider-xai","provider-ollama",
                 "provider-azure","provider-perplexity","provider-together"]
```

`providers/mod.rs::build_provider_map(cfg: &LlmConfig)`:
- Walks the unique `provider` strings in `cfg.aliases`.
- For each, conditionally compiles a constructor: `"openai" => RigAdapter::new(rig::providers::openai::Client::from_env().completion_model(model_id))`.
- Returns `HashMap<String, Arc<dyn CompletionProvider>>` — same shape `LlmRegistry::from_config` already expects, so the registry itself does **not** change.

#### B.3 Boot validation
- `verify_llm_providers` ([llm/registry.rs](apps/backend/crates/agent-core/src/llm/registry.rs)) is unchanged in behaviour — still alias→provider checks, no network.
- New `validate_provider_credentials_optional` (gated behind a `--probe-providers` CLI flag) calls a 1-token completion; **off by default** to honour principle 5.

#### B.4 Config surface
Extend [common::config::LlmConfig](apps/backend/crates/common/src/config.rs) example block:
```toml
[llm]
default = "fast"

[llm.aliases.fast]    provider = "groq"      model = "llama-3.3-70b-versatile"
[llm.aliases.smart]   provider = "anthropic" model = "claude-sonnet-4-6"
[llm.aliases.local]   provider = "ollama"    model = "qwen2.5:14b"
[llm.aliases.vision]  provider = "anthropic" model = "claude-opus-4-7"
```

#### B.5 Acceptance
- All 10 provider feature flags compile (`cargo check --features all-providers`).
- One integration test per provider behind `#[cfg(feature = "provider-X")]` using `wiremock`.
- Existing `routes/v1/agent.rs` chat tests pass unchanged (registry contract preserved).

---

### Phase C — Multimodal: image-gen, TTS, transcription via Rig traits

**Outcome:** `LlmRegistry` resolves not only completion bindings but also **image / audio / transcription** bindings with the same alias mechanism. Web + shell expose them through new `@conusai/sdk` methods.

#### C.1 Backend: registry split

Add to [llm/types.rs](apps/backend/crates/agent-core/src/llm/types.rs):
```rust
pub enum ModalityKind { Completion, ImageGen, AudioGen, Transcription }
pub struct ModalBinding { pub provider: String, pub model: String, pub kind: ModalityKind }
```

Extend `LlmRegistry`:
```rust
image_models: HashMap<String, Arc<dyn rig::image_generation::ImageGenerationModelDyn>>,
audio_models: HashMap<String, Arc<dyn rig::audio_generation::AudioGenerationModelDyn>>,
transcription_models: HashMap<String, Arc<dyn rig::transcription::TranscriptionModelDyn>>,
```

Resolution order mirrors the existing 4-step `resolve_binding`, scoped per kind. Plan limits add `tenant.plan.modalities_allowed: BitFlags<ModalityKind>`.

#### C.2 New capabilities

Add three built-in capabilities under `capabilities/builtin/`:
- `image_generate.rs` — wraps `ImageGenerationModelDyn::generate(prompt, opts)`; output stored via existing `RustFsContentStore`, returns `{ artifact_id, mime, prompt }`.
- `audio_speak.rs` — TTS via `AudioGenerationModelDyn`; streamed bytes piped into the existing artifact store.
- `audio_transcribe.rs` — accepts `{ artifact_id }` or raw bytes, returns `{ text, segments[], language }`.

Each capability is a normal TOML manifest under [apps/backend/capabilities/](apps/backend/capabilities/) so the existing factory chain (`McpFactory → WasmFactory → ChainFactory → BuiltinFactory`) picks them up unchanged.

#### C.3 Streaming protocol delta

Extend `ChatStreamDelta` (and the `chat:chunk:<id>` event payload) in [packages/types](packages/types/) with a tagged union (2026 streaming-API convention — `#[serde(tag = "kind")]` on the Rust mirror keeps forward-compatibility when new chunk kinds land):
```ts
type ChunkPayload =
  | { kind: "text"; delta: string }
  | { kind: "tool_call"; … }
  | { kind: "image"; artifactId: string; mime: "image/png" | "image/jpeg" }
  | { kind: "audio"; artifactId: string; mime: "audio/mpeg" | "audio/wav" }
  | { kind: "transcript"; segments: TranscriptSegment[] };
```
Rust mirror in [agent-gateway/src/routes/v1/agent.rs](apps/backend/crates/agent-gateway/src/routes/v1/agent.rs):
```rust
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChunkPayload { Text { delta: String }, ToolCall { … }, Image { … }, Audio { … }, Transcript { … } }
```

#### C.4 SDK + UI

- `@conusai/sdk`: add `sdk.images.generate`, `sdk.audio.speak`, `sdk.audio.transcribe` plus stream variants returning `ReadableStream<ChunkPayload>`.
- `packages/ui/src/lib/capabilities/`: add three renderers (`ImageArtifact.svelte`, `AudioArtifact.svelte`, `TranscriptArtifact.svelte`) registered in the capability renderer registry.
- Renderers honour reduced-motion (no waveform animation when `prefers-reduced-motion: reduce`).

#### C.5 Browser-shell

- Reuse the existing native chat bridge — image/audio chunks ride the same `chat:chunk:<id>` event.
- Add Tauri invoke handlers `record_microphone_chunk` / `play_audio_artifact` that call into existing media plugins (no new Rust crates required for desktop; iOS/Android use platform recorders via `tauri-plugin-dialog` + a tiny `media.rs` module).

#### C.6 Acceptance
- E2E (`e2e/web/multimodal.spec.ts`): submit "draw a sunset" → assert `image` chunk + artifact preview renders.
- E2E shell (`e2e/shell-macos/multimodal.spec.ts`): TTS round-trip plays.
- Reduced-motion test confirms no waveform animation.
- `cargo test -p agent-core --features provider-openai` covers image-gen unit.

---

### Phase D — Long-term Memory: Rig `memory` feature

**Outcome:** Delete `memory/context_builder.rs` and `memory/truncator.rs`. Memory recall becomes a tool the agent calls (or auto-RAG) backed by the same `VectorStoreIndex` from Phase A, scoped by tenant.

#### D.1 New module `agent_core::memory_v2`

```
agent-core/src/memory_v2/
├── mod.rs
├── episodic.rs     # turn-by-turn recall (per thread)
├── semantic.rs     # cross-thread facts (per tenant)
└── workspace.rs    # replaces ContextBuilder ancestor walk
```

All three implement the same pattern:
```rust
pub struct EpisodicMemory<I: VectorStoreIndex> { index: I, embedder: ... }
impl<I> EpisodicMemory<I> {
    pub async fn record(&self, tenant: &TenantContext, thread: Ulid, turn: TurnSummary);
    pub async fn recall(&self, tenant: &TenantContext, query: &str, k: usize) -> Vec<MemoryHit>;
}
```

Tenant + scope (`thread_id`, `workspace_node_id`) live in record metadata; recall pushes them into the Rig filter expression.

#### D.2 Workspace context replacement

`workspace.rs::build_for_node` keeps the same signature as today's `ContextBuilder::build_for_node` so callers in `agent-gateway::routes::v1::agent` don't change. Internally:
- Persist each ancestor `CONTEXT.md` / `README.md` once (idempotent) into the memory index with `kind = "workspace_context"`, `node_id` metadata.
- Build the system preamble by issuing a single `index.top_n(query, k)` filtered by the ancestor `node_id` set — drops the manual ancestor loop and the `OldestFirstTruncator`.
- Truncation is delegated to Rig's built-in `max_chars` clamp on the recall result.

#### D.3 Auto-recall hook

Add a Rig `PromptHook` (alongside `TracingHook` and `PermissionHook` in [agent/hooks.rs](apps/backend/crates/agent-core/src/agent/hooks.rs)):
```rust
pub struct MemoryRecallHook { index: Arc<dyn VectorStoreIndexDyn>, tenant: TenantContext, thread: Ulid }
impl<M: CompletionModel> PromptHook<M> for MemoryRecallHook {
    async fn on_completion_call(&self, ctx: &mut PromptContext<M>) -> HookAction { ... inject top-3 memories as system message ... }
}
```

This makes long-term memory **transparent** to chains and capabilities — they don't need to call a `memory_search` tool explicitly.

#### D.4 Tool surface

Still expose an explicit `memory_search` builtin capability so chains and external clients can query it deliberately (mirrors how `fs__read_file` is both used implicitly by the agent and explicitly by chains).

#### D.5 Migration of existing data

- One-shot migration script `scripts/memory_migrate.rs`: walks current `redb_metadata` workspace nodes, re-embeds CONTEXT.md/README.md, inserts into the new index. Idempotent — re-runnable.
- Keep `redb_metadata` as the canonical KV; only the **vectorised** copy moves into the Rig index.

#### D.6 Cleanup

Delete after grep:
- `memory/context_builder.rs`
- `memory/truncator.rs`
- The `OldestFirstTruncator` re-export in `memory/mod.rs`.

#### D.7 Acceptance
- Existing chat E2E (`e2e/web/agent_chat.spec.ts`) passes unchanged — memory injection is invisible to user-facing assertions.
- New unit test: `memory_v2::episodic::recall_isolation` proves tenant-A queries never return tenant-B turns.
- Latency: `on_completion_call` recall ≤ 5 ms P50 with 10k memories per tenant (LanceDB local).

---

## 3. Cross-Cutting Workstreams

These run in parallel with Phases A–D.

### 3.1 Telemetry
- New OTel attributes: `rig.provider`, `rig.model`, `rig.modality`, `vector.backend`, `memory.scope`. Define once in [common::metrics](apps/backend/crates/common/src/metrics.rs).
- Histograms: `rig_completion_duration_ms`, `vector_topk_duration_ms`, `memory_recall_duration_ms`.

### 3.2 Error mapping
- Extend `agent/runtime.rs::map_rig_error` table to cover the new error substrings emitted by image-gen / audio / transcription providers (`"image generation failed"`, `"audio decode"`, etc.). Add a regression test per substring.

### 3.3 Config + secrets
- Extend `LlmConfig` schema; add `--print-effective-config` CLI flag for ops.
- Document new env vars in [docs/ops/](docs/ops/) (`OPENAI_API_KEY`, `GROQ_API_KEY`, `OLLAMA_HOST`, …).

### 3.4 Documentation
- After **each** phase ships, append a "Migration log" entry to [docs/arch.md](docs/arch.md) and update [docs/arch2.md](docs/arch2.md) §4.4 (Rig usage) with the now-current trait surface.
- Update [docs/capability-authoring-guide.md](docs/capability-authoring-guide.md) with the new image/audio/transcription manifest examples.

### 3.5 Test matrix
| Layer | Tooling | Phase coverage |
| --- | --- | --- |
| Rust unit | `cargo test -p agent-core` | A, B, D |
| Rust integ | `cargo test -p agent-gateway` (wiremock) | A, B, C |
| Web E2E | `pnpm -C apps/web exec playwright test` | C |
| Shell E2E (macOS) | `pnpm -C e2e/shell-macos test` | C |
| Bench | `cargo bench -p agent-core` | A, D |

---

## 4. Risk Register

| Risk | Mitigation |
| --- | --- |
| LanceDB embedded files conflict with existing redb workspace dir | Place LanceDB under `<workspace>/.conusai/vector/` with a marker file; refuse to start if a v0 Qdrant collection exists without explicit `--migrate` flag. |
| Rig 0.36 trait churn between minor releases | Pin `rig-core = "=0.36"`, add a `cargo deny` rule, and run a **quarterly Rig compatibility matrix** CI job against the latest `0.36.x` patch. Bump deliberately. |
| Provider feature flags cause combinatorial CI cost | CI matrix runs only `default` + `all-providers`; per-provider tests gate on `[ci skip-provider-X]` markers. |
| Memory hook injects irrelevant context (recall noise) | `max_distance` threshold (mirrors current `0.38`) plus per-tenant calibration job in `evals/`. |
| Custom `EmbeddingService` removal breaks `coco_indexer` mid-flight | Phase A.6 keeps a thin compat shim; remove only after `coco_indexer` is ported to `rig::embeddings::EmbeddingsBuilder`. |
| Streaming chunk schema change breaks older shell builds | Bump `@conusai/types` major; gateway negotiates protocol version via existing `X-Conus-Protocol` header; shell falls back to text-only chunks if older. |

---

## 5. Sequenced Execution Checklist

> One PR per checkbox. Each PR carries the "Guiding Constraints" verification block in its description.

**Phase A — Vector / LanceDB**
- [ ] A.1 Add `rig-lancedb`, `rig-qdrant`, `lancedb` to workspace + features.
- [ ] A.2 Create `agent-core/src/vector/` with `schema.rs`, `lancedb.rs`, `qdrant.rs`.
- [ ] A.3 Refactor `SemanticCapabilityRouter` to consume `VectorStoreIndexDyn`; delete moka cache.
- [ ] A.4 Replace `capabilities/embedding.rs` with `EmbeddingsBuilder` in gateway bootstrap.
- [ ] A.5 Rewire `AppState`; port `ConstEmbedder` test double to `rig::embeddings::EmbeddingModel`.
- [ ] A.6 Delete `store/qdrant_vector.rs`, `vector_store/mod.rs`, `capabilities/embedding.rs`.
- [ ] A.7 Add `benches/router_topk.rs`; assert tenant-isolation in unit test.

**Phase B — Providers**
- [ ] B.1 Implement `RigAdapter<M>` shim; delete `anthropic.rs`.
- [ ] B.2 Add provider feature flags; implement `build_provider_map`.
- [ ] B.3 `--probe-providers` CLI flag (off by default).
- [ ] B.4 Extend example `[llm.aliases]` blocks; document env vars.
- [ ] B.5 One wiremock integ test per provider feature.

**Phase C — Multimodal**
- [ ] C.1 Extend `LlmRegistry` with image/audio/transcription maps.
- [ ] C.2 Add `image_generate`, `audio_speak`, `audio_transcribe` builtin capabilities + manifests.
- [ ] C.3 Extend `ChatStreamDelta` / `ChunkPayload` types (Rust + TS).
- [ ] C.4 SDK methods + UI artifact renderers (reduced-motion safe).
- [ ] C.5 Tauri media handlers; native chat bridge passes new chunk kinds through.
- [ ] C.6 Web + shell E2E specs.

**Phase D — Memory**
- [ ] D.1 Create `memory_v2/` (episodic, semantic, workspace).
- [ ] D.2 Implement `MemoryRecallHook`; wire into `AgentBuilder`.
- [ ] D.3 Expose `memory_search` builtin capability.
- [ ] D.4 Migration script `scripts/memory_migrate.rs`.
- [ ] D.5 Delete `memory/context_builder.rs`, `memory/truncator.rs`.
- [ ] D.6 Tenant-isolation unit + recall-latency bench.

**Cross-cutting (per phase)**
- [ ] Telemetry attributes added.
- [ ] `map_rig_error` regression tests extended.
- [ ] `docs/arch.md` + `docs/arch2.md` migration log appended.
- [ ] `docs/capability-authoring-guide.md` updated.
- [ ] PR description includes the **Rig surface line**: `Rig surface used: <traits/methods> (verified against docs.rig.rs 0.36)`.

**Post-migration audit**
- [ ] `cargo udeps` clean (no unused workspace deps).
- [ ] `cargo machete` clean (no unused per-crate deps).
- [ ] `rg "QdrantVectorStore|EmbeddingService|ContextBuilder|OldestFirstTruncator|AnthropicProvider" apps/backend/crates/agent-core/src` returns zero hits — **Rig purity audit**.

---

## 6. Definition of Done (whole programme)

1. Zero references to `QdrantVectorStore`, custom `EmbeddingService`, `ContextBuilder`, `OldestFirstTruncator`, or per-provider `CompletionProvider` impls remain in `agent-core` (verified by `rg` — see Rig purity audit checklist).
2. `cargo test --workspace --features all-providers` green.
3. `cargo udeps` and `cargo machete` clean.
4. `pnpm test` and all Playwright suites (`web`, `shell-macos`, `ios`) green.
5. New OTel histograms visible in `/metrics`.
6. Net LOC delta: ≥ 1,200 lines deleted from `agent-core` (per `rig-features.md` estimate).
7. Quarterly Rig compatibility matrix CI job is wired and green on the latest `0.36.x`.
8. ConusAI is a **pure Rig 0.36 platform** with no custom glue in the four target areas.
