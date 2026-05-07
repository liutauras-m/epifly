# Plan: Generic, Extensible Agent System (v0.3.2)

> **Goal:** Evolve the v0.3.1 agent into a **generic, prompt-driven, semantically-routed** system that supports **10,000+ stateless capabilities** (ERP, accounting, generic domain logic) with **zero breaking changes**.
>
> **Strategy:** Build on the existing `CapabilityProvider` / `CapabilityFactory` / `PgVectorStore` foundation. Add three small composable layers — **semantic router**, **dynamic prompts**, **namespaces** — and wire them into `AgentBuilder` + the gateway, leaning on Rig 0.36 primitives wherever they exist.
>
> **North-star principle:** *Never send 10k tools to the LLM.* Always semantic-prefilter to top-K (≤30) capabilities per turn.
>
> **Effort budget:** 35–45 AI-hours (~220k–280k tokens). Phase 1 (6–8h), Phase 2 (12–15h), Phase 3 (8–10h), Phase 4 (6–8h), Phase 5–7 (3–4h).

---

## 0. Current State Snapshot (verified in code)

Branch `feat/v0.3-rig-workspace-wasi`, commit `e666eae`:

| Layer | Status | Path |
|---|---|---|
| `CapabilityProvider` trait | ✅ | [apps/backend/crates/agent-core/src/tools/provider.rs](apps/backend/crates/agent-core/src/tools/provider.rs) |
| `CapabilityFactory` trait + 4 factories (Mcp/Wasm/Chain/Builtin) | ✅ | [apps/backend/crates/agent-core/src/tools/providers/](apps/backend/crates/agent-core/src/tools/providers) |
| `ToolRegistry` + `CapabilityCard` + filesystem store | ✅ | [apps/backend/crates/agent-core/src/tools/registry.rs](apps/backend/crates/agent-core/src/tools/registry.rs), [store.rs](apps/backend/crates/agent-core/src/tools/store.rs) |
| `PromptChainCapability` (static templates from `capability.toml`) | ✅ | [apps/backend/crates/agent-core/src/chains/llm_chain.rs](apps/backend/crates/agent-core/src/chains/llm_chain.rs) |
| `PgVectorStore` (`top_n_capabilities`, diskann) | ✅ | [apps/backend/crates/agent-core/src/vector_store/postgres.rs](apps/backend/crates/agent-core/src/vector_store/postgres.rs) |
| `EmbeddingService` (OpenAI + local fastembed) | ✅ | [apps/backend/crates/agent-core/src/indexing/embedding_service.rs](apps/backend/crates/agent-core/src/indexing/embedding_service.rs) |
| `CapabilityAdmin` HTTP CRUD + hot-reload + test-invoke | ✅ | [apps/backend/crates/agent-core/src/tools/admin.rs](apps/backend/crates/agent-core/src/tools/admin.rs), [admin_capabilities.rs](apps/backend/crates/agent-gateway/src/routes/admin_capabilities.rs) |
| `capability_embeddings` table (vector(768) + diskann) | ✅ | [docker/init/02-schema.sql](docker/init/02-schema.sql) |
| Rig 0.36 (`rig::completion`, `rig::providers::anthropic`, `rig-postgres`) | ✅ | [apps/backend/crates/agent-core/src/llm/providers/anthropic.rs](apps/backend/crates/agent-core/src/llm/providers/anthropic.rs), [vector_store/postgres.rs](apps/backend/crates/agent-core/src/vector_store/postgres.rs) |
| `blake3`, `bon` workspace deps | ✅ | [Cargo.toml](Cargo.toml) lines 66, 90 |
| Realtime PG `LISTEN/NOTIFY` | ✅ | [apps/backend/crates/agent-core/src/realtime/mod.rs](apps/backend/crates/agent-core/src/realtime/mod.rs) |

### Gaps to close

1. **No semantic router** — capabilities are looked up by exact flat name (`{cap}__{tool}`); the LLM sees all enabled tools every turn.
2. **No dynamic prompts** — `PromptChainCapability` reads `capability.toml` once at registration; no DB-backed prompt loading or versioning.
3. **No namespaces / tags surfaced for routing** — flat naming forbids `accounting.invoice.*`-style organisation.
4. **`CapabilityFactory` is not bulk-friendly** — generating 8000 ERP cards from DB rows requires bespoke code today; no `load_batch`.
5. **`AgentBuilder` is not wired to capabilities** — agent loop dispatches via the gateway, not via a builder API. Embedded users have no path.
6. **`moka` not yet in workspace** — needed for the router cache (add it once in [Cargo.toml](Cargo.toml)).

---

## 1. Architecture (target v0.3.2)

```
┌──────────────────────────────────────────────────────────────┐
│                       AgentBuilder                           │
│  .with_semantic_router(SemanticCapabilityRouter)  ◄── new    │
│  .with_namespaces([NamespaceFilter::Prefix("erp")])  ◄── new │
└──────────────┬───────────────────────────────────────────────┘
               │ build()  → wraps router as rig::ToolProvider
               ▼
┌──────────────────────────────────────────────────────────────┐
│              SemanticCapabilityRouter (NEW)                  │
│  1. embed(query) [moka-cached, blake3-keyed]                 │
│  2. PgVectorStore.top_n_capabilities_filtered(emb, K, ns)    │
│  3. namespace + tag filter (Exact | Prefix | AnyOf)          │
│  4. expose top-K as Anthropic tool defs (rig ToolProvider)   │
│  5. dispatch invoke() → ToolRegistry → Provider              │
└────────┬─────────────────────────────────┬───────────────────┘
         │                                 │
         ▼                                 ▼
┌──────────────────┐           ┌────────────────────────────────┐
│  ToolRegistry    │           │  PgVectorStore                 │
│  (existing)      │           │  capability_embeddings         │
│  + bulk loader   │           │  + namespace + tags (NEW cols) │
└────────┬─────────┘           └────────────────────────────────┘
         │
         ▼
┌──────────────────────────────────────────────────────────────┐
│  Providers (CapabilityProvider impls)                        │
│  • BuiltinFactory       (Rust, deterministic)                │
│  • WasmFactory          (sandboxed, deterministic)           │
│  • McpFactory           (external services)                  │
│  • ChainFactory         (PromptChainCapability — static)     │
│  • DynamicPromptFactory (NEW — DB-backed prompts, versioned) │
│  • CapabilitySpecFactory (NEW — bulk-generated, BulkCapabilityFactory) │
└──────────────────────────────────────────────────────────────┘
```

**Hybrid policy** (industry-standard 2026):
- **Deterministic core** (double-entry, tax, ledger posting, reconciliation) → Rust/WASM providers.
- **High-level workflows** (invoice → GL → approval → report) → `PromptChainCapability` + sub-agents.
- **Domain rule changes** (new GL account, new tax jurisdiction) → DB row insert → `DynamicPromptCapability` reloads — **zero Rust rebuild, zero restart**.

**Naming convention (final):** `SemanticCapabilityRouter` lives in `tools/semantic_router.rs`. Distinguishes from future `GraphCapabilityRouter` (orchestrator pattern) in v0.4.

---

## 2. Step-by-Step Implementation Plan

### Phase 1 — Namespaces & multi-tag filtering (foundation)

**Why first:** Every later phase routes by namespace + tag. Doing this last forces rework.

#### 1.1 Extend `ToolManifest` & `CapabilityCard`

- File: [apps/backend/crates/agent-core/src/tools/manifest.rs](apps/backend/crates/agent-core/src/tools/manifest.rs)
  - Add `pub namespace: Option<String>` to `ToolManifest` (TOML field `namespace = "accounting.invoice"`).
  - **Keep `tags: Vec<String>` (already present)** as the multi-namespace tagging mechanism; namespace is the single primary, tags are secondary axes — matches the `tags` column in `capability_specs`.
  - Helper: `pub fn namespace(&self) -> &str { self.namespace.as_deref().unwrap_or("") }`.
- File: [apps/backend/crates/agent-core/src/tools/card.rs](apps/backend/crates/agent-core/src/tools/card.rs)
  - Re-export `card.namespace()` and `card.tags()`.
- File: [apps/backend/crates/agent-core/src/tools/validator.rs](apps/backend/crates/agent-core/src/tools/validator.rs)
  - Add `validate_namespace(&str)` with regex `^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*){0,5}$` (≤6 segments, slug-only). Call it from `validate_manifest`.

#### 1.2 `NamespaceFilter` enum (canonical filter API)

- New file: `apps/backend/crates/agent-core/src/tools/namespace.rs`
  ```rust
  #[derive(Debug, Clone, Default)]
  pub enum NamespaceFilter {
      #[default]
      Any,
      Exact(String),
      Prefix(String),                 // "accounting." matches "accounting.gl", "accounting.ap"
      AnyOf(Vec<NamespaceFilter>),
  }

  impl NamespaceFilter {
      pub fn to_sql_predicate(&self, col: &str, bind_offset: usize) -> (String, Vec<String>);
      pub fn matches(&self, ns: &str) -> bool;
  }
  ```
  Used by both `PgVectorStore` (SQL) and the in-memory cache layer.

#### 1.3 Persist namespace + tags in DB

- New migration `apps/backend/crates/common/migrations/20260507000000_capability_namespaces.up.sql`:
  ```sql
  ALTER TABLE capability_embeddings
    ADD COLUMN namespace TEXT NOT NULL DEFAULT '',
    ADD COLUMN tags      TEXT[] NOT NULL DEFAULT '{}';
  CREATE INDEX cap_embed_ns_idx   ON capability_embeddings (namespace);
  CREATE INDEX cap_embed_tags_idx ON capability_embeddings USING gin (tags);
  ```
- Mirror columns in [docker/init/02-schema.sql](docker/init/02-schema.sql).

#### 1.4 Update embedding upsert + filtered query

- File: [apps/backend/crates/agent-core/src/vector_store/postgres.rs](apps/backend/crates/agent-core/src/vector_store/postgres.rs)
  - Extend `upsert_capability_embedding(..., namespace, tags)`.
  - Add `top_n_capabilities_filtered(embedding, k, ns: &NamespaceFilter, tags_any: &[String]) -> Vec<CapabilityHit>`. Builds SQL via `NamespaceFilter::to_sql_predicate` and `tags && $N::text[]` for tag-any matching.
  - Keep existing `top_n_capabilities(emb, k)` as a thin wrapper calling `_filtered(emb, k, &NamespaceFilter::Any, &[])`.
- Update sync path triggered by `CapabilityAdmin::create/update/reload` to pass namespace + tags.

#### 1.5 Lightweight namespace tree for admin UX

- In `ToolRegistry`: maintain `namespace_index: indexmap::IndexMap<String, Vec<String>>` (segment → child segments), rebuilt on register/reload. Powers admin autocomplete (`GET /admin/capabilities/namespaces?prefix=acc`).

**Acceptance:** Capability declaring `namespace = "accounting.invoice"` + `tags = ["v1","priority"]` round-trips through TOML → registry → DB; `top_n_capabilities_filtered(_, 20, &Prefix("accounting"), &["priority"])` returns it.

---

### Phase 2 — `SemanticCapabilityRouter`

#### 2.1 Define the router

- New file: `apps/backend/crates/agent-core/src/tools/semantic_router.rs`
  ```rust
  use bon::Builder;

  #[derive(Builder)]
  pub struct SemanticRouterConfig {
      #[builder(default = 20)]   pub top_k: usize,           // hard max 50
      #[builder(default)]        pub namespace: NamespaceFilter,
      #[builder(default)]        pub tags_any: Vec<String>,
      #[builder(default = 0.65)] pub max_distance: f64,      // cosine; reject far hits
      #[builder(default)]        pub include_always: Vec<String>,
      #[builder(default = 60)]   pub cache_ttl_secs: u64,
  }

  pub struct SemanticCapabilityRouter {
      registry: Arc<RwLock<ToolRegistry>>,
      vector_store: Arc<PgVectorStore>,
      embedder: Arc<dyn EmbeddingService>,
      cfg: SemanticRouterConfig,
      cache: moka::future::Cache<[u8; 32], Arc<Vec<CapabilityHit>>>,  // blake3 keys
      metrics: Arc<RouterMetrics>,
  }

  impl SemanticCapabilityRouter {
      pub async fn select(&self, query: &str, tenant: &TenantContext) -> Result<Vec<Arc<dyn CapabilityProvider>>>;
      pub async fn tool_definitions(&self, query: &str, tenant: &TenantContext) -> Result<Vec<Value>>;
      pub async fn invoke(&self, tool_name: &str, input: &Value, tenant: Option<&TenantContext>) -> Result<Value>;
  }
  ```

#### 2.2 Cache key (collision-resistant)

```rust
fn cache_key(tenant_id: &str, query: &str, cfg: &SemanticRouterConfig) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(tenant_id.as_bytes());
    h.update(query.as_bytes());
    h.update(&cfg.top_k.to_le_bytes());
    // include namespace + tags + max_distance bytes
    *h.finalize().as_bytes()
}
```
Add `moka = { version = "0.12", features = ["future"] }` to workspace [Cargo.toml](Cargo.toml).

#### 2.3 Rig integration — implement `ToolProvider`

- Wrap `SemanticCapabilityRouter` with a `RigToolProviderAdapter` that implements rig 0.36's tool-resolution surface (the trait in `rig::completion::ToolDefinition` / `rig::tool` module — exact name confirmed at implementation time against the pinned `rig-core = "0.36"`).
- Adapter responsibilities:
  - `definitions(prompt) -> Vec<rig::completion::ToolDefinition>` calls `router.tool_definitions(prompt, tenant)`.
  - `call(name, args) -> Result<String>` calls `router.invoke(name, &args, Some(&tenant))`.
- This makes the router pluggable into both:
  - Our hand-rolled gateway loop ([routes/agent.rs](apps/backend/crates/agent-gateway/src/routes/agent.rs)).
  - Any future `rig::AgentBuilder::tools(adapter)` user.

#### 2.4 Wire into the agent loop & builder

- File: [apps/backend/crates/agent-gateway/src/routes/agent.rs](apps/backend/crates/agent-gateway/src/routes/agent.rs)
  - Replace "list all enabled tools" with:
    1. Read latest user message → `router.tool_definitions(msg, tenant).await?`.
    2. Pass that subset to the LLM completion request.
    3. On tool-call → `router.invoke(name, input, Some(&tenant)).await?`.
- File: [apps/backend/crates/agent-gateway/src/state.rs](apps/backend/crates/agent-gateway/src/state.rs)
  - Construct `Arc<SemanticCapabilityRouter>` once at boot; add to `AppState`.
- File: [apps/backend/crates/agent-core/src/agent/builder.rs](apps/backend/crates/agent-core/src/agent/builder.rs)
  - Add `with_semantic_router(Arc<SemanticCapabilityRouter>)`. Builder internally wires the adapter as **both** rig tool provider **and** context source — unifying gateway and embedded paths.

#### 2.5 Tower middleware for quotas

- New: `apps/backend/crates/agent-gateway/src/mw/router_quota.rs`
  - Tower layer that reads `TenantContext` and enforces `max_tools_per_turn` / `max_invokes_per_turn` (default 25 / 10) before calling the router. Reuses existing tower stack — no new framework.

**Acceptance:** Agent turn with 10k embeddings → trace shows ≤K tool definitions sent to Anthropic; p95 router overhead < 15 ms warm cache, < 25 ms cold.

---

### Phase 3 — `DynamicPromptCapability`

#### 3.1 Schema for DB-backed, versioned prompts

- Migration `20260507000100_dynamic_prompts.up.sql`:
  ```sql
  CREATE TABLE dynamic_prompts (
      capability_name TEXT NOT NULL,
      version         INT  NOT NULL DEFAULT 1,
      system_prompt   TEXT,
      user_template   TEXT NOT NULL,        -- minijinja
      few_shot        JSONB NOT NULL DEFAULT '[]',
      output_schema   JSONB,
      model           TEXT NOT NULL,
      max_tokens      INT  NOT NULL DEFAULT 1024,
      vision          BOOL NOT NULL DEFAULT false,
      updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
      PRIMARY KEY (capability_name, version)
  );
  CREATE INDEX dyn_prompts_latest_idx ON dynamic_prompts (capability_name, version DESC);
  ```
  `(name, version)` PK preserves history for `?version=N` retrieval.

#### 3.2 Extract shared chain executor

- Refactor [apps/backend/crates/agent-core/src/chains/llm_chain.rs](apps/backend/crates/agent-core/src/chains/llm_chain.rs):
  - Extract `pub async fn run_chain(cfg: &LlmChainConfig, ctx: &Value, llm: &LlmRegistry) -> Result<Value>` into new `apps/backend/crates/agent-core/src/chains/executor.rs`.
  - `PromptChainCapability::invoke` → calls `executor::run_chain(&self.cfg, &ctx, &self.llm)`.

#### 3.3 Provider

- New file: `apps/backend/crates/agent-core/src/chains/dynamic_prompt.rs`
  ```rust
  pub struct DynamicPromptCapability {
      manifest: ToolManifest,
      llm: Arc<LlmRegistry>,
      pool: PgPool,
      cache: moka::future::Cache<String, Arc<LlmChainConfig>>,  // key = "{name}:{version}"
  }

  #[async_trait]
  impl CapabilityProvider for DynamicPromptCapability {
      async fn invoke(&self, _tool: &str, input: &Value, tenant: Option<&TenantContext>) -> Result<Value> {
          let cfg = self.load_latest().await?;          // SELECT … ORDER BY version DESC LIMIT 1
          let ctx = json!({ "input": input, "tenant": tenant });
          executor::run_chain(&cfg, &ctx, &self.llm).await
      }
  }
  ```

#### 3.4 Factory + ToolKind

- Extend `ToolKind` enum: add `DynamicPrompt`.
- New file: `apps/backend/crates/agent-core/src/tools/providers/dynamic_prompt.rs` with `DynamicPromptFactory { pool, llm }`.
- Register in `ToolRegistry::with_default_factories(...)`.

#### 3.5 Admin endpoints

- File: [admin_capabilities.rs](apps/backend/crates/agent-gateway/src/routes/admin_capabilities.rs):
  - `PUT /admin/capabilities/:name/prompt` — INSERT new row with `version = max+1`. Triggers re-embed **only when `embedding_text()` changes** (delta optimisation).
  - `GET /admin/capabilities/:name/prompt?version=N` — defaults to latest.
  - `GET /admin/capabilities/:name/prompt/versions` — list versions.

**Acceptance:** Edit a prompt via admin → next turn uses new prompt without restart; `?version=N` retrieves prior versions; cache invalidates on upsert.

---

### Phase 4 — Bulk capability factory (domain-neutral, ERP-first vertical)

#### 4.1 Source-of-truth table

- Migration `20260507000200_capability_specs.up.sql`:
  ```sql
  CREATE TABLE capability_specs (
      id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
      namespace     TEXT NOT NULL,                  -- e.g. erp.po, crm.lead, accounting.gl
      tool_name     TEXT NOT NULL,
      description   TEXT NOT NULL,
      input_schema  JSONB NOT NULL,
      output_schema JSONB,
      strategy      TEXT NOT NULL,                  -- 'wasm' | 'prompt' | 'native'
      payload       JSONB NOT NULL,                 -- prompt id, wasm hash, etc.
      tags          TEXT[] NOT NULL DEFAULT '{}',
      enabled       BOOL NOT NULL DEFAULT true,
      updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
      UNIQUE (namespace, tool_name)
  );

  -- LISTEN/NOTIFY for hot reload
  CREATE OR REPLACE FUNCTION notify_capability_specs_changed() RETURNS trigger AS $$
  BEGIN
      PERFORM pg_notify('capability_specs_changed',
          json_build_object('namespace', NEW.namespace, 'tool_name', NEW.tool_name, 'op', TG_OP)::text);
      RETURN NEW;
  END $$ LANGUAGE plpgsql;
  CREATE TRIGGER capability_specs_changed_trg
      AFTER INSERT OR UPDATE OR DELETE ON capability_specs
      FOR EACH ROW EXECUTE FUNCTION notify_capability_specs_changed();
  ```

#### 4.2 `BulkCapabilityFactory` trait (ergonomics for 10k loads)

- File: [apps/backend/crates/agent-core/src/tools/provider.rs](apps/backend/crates/agent-core/src/tools/provider.rs):
  ```rust
  #[async_trait]
  pub trait BulkCapabilityFactory: CapabilityFactory {
      /// Load many capabilities efficiently (batched embeddings, single tx).
      async fn load_batch(&self, into: &mut ToolRegistry) -> Result<usize>;
  }
  ```
- `ToolRegistry::register_bulk_factory(...)` stores it for invocation by the gateway boot path.

#### 4.3 `CapabilitySpecFactory`

- New file: `apps/backend/crates/agent-core/src/tools/providers/capability_spec.rs`
  ```rust
  #[derive(bon::Builder)]
  pub struct CapabilitySpecFactory {
      pool: PgPool,
      llm: Arc<LlmRegistry>,
      embedder: Arc<dyn EmbeddingService>,
      vector_store: Arc<PgVectorStore>,
      #[builder(default = 256)] batch_size: usize,
  }
  ```
- `load_batch`:
  1. Stream `capability_specs WHERE enabled` in chunks of `batch_size`.
  2. Map each row → `CapabilityCard` via `CapabilitySpecMapper` (generic over strategy: wasm | prompt | native).
  3. Batch-call `embedder.embed_batch(...)` for the chunk.
  4. Single `INSERT … ON CONFLICT (capability_id) DO UPDATE` into `capability_embeddings`.
  5. Insert provider into the registry.

#### 4.4 Hot-reload via `LISTEN/NOTIFY`

- Use the existing realtime infrastructure ([apps/backend/crates/agent-core/src/realtime/mod.rs](apps/backend/crates/agent-core/src/realtime/mod.rs)) — already subscribes to PG NOTIFY.
- Add channel handler `capability_specs_changed` → calls `factory.reload_one(namespace, tool_name)` which updates the registry **and** re-embeds just that row.

**Acceptance:** Insert 10k rows → boot job populates `capability_embeddings` (10k rows) in < 30 s; subsequent NOTIFY → registry updates in < 200 ms; semantic top-20 query returns in < 15 ms.

---

### Phase 5 — Observability & guardrails

- **OpenTelemetry GenAI semantic conventions** (2026 standard):
  - `gen_ai.tool.calls` (counter, label `gen_ai.tool.name`)
  - `gen_ai.semantic_router.top_k` (histogram)
  - `gen_ai.semantic_router.distance` (histogram)
  - `gen_ai.semantic_router.cache_hit` (counter)
- **Tracing spans**:
  - `semantic_router.select` { tenant_id, namespace, top_k, hit_count, cache_hit, distance_min, distance_max }
  - `semantic_router.invoke` { tool_name, capability_kind, outcome }
  - `dynamic_prompt.load` { capability_name, version, cache_hit }
- **Metrics** (extend [apps/backend/crates/common/src/metrics.rs](apps/backend/crates/common/src/metrics.rs)):
  - `capability_router_select_seconds` (histogram, labels: namespace, hit_count_bucket).
  - `capability_invoke_seconds` (histogram, labels: capability, kind, outcome).
- **Quotas** (read from `TenantContext` — already plumbed):
  - `max_tools_per_turn` (default 25), `max_invokes_per_turn` (default 10), enforced in tower middleware (Phase 2.5).
  - When `TenantConfig` table arrives in v0.4, quotas are read from there.
- **Audit**: extend `AuditEvent` (in [apps/backend/crates/agent-core/src/memory/postgres_audit_store.rs](apps/backend/crates/agent-core/src/memory/postgres_audit_store.rs)) with `selected_top_k: usize`, `selected_capabilities: Vec<String>`, `cache_hit: bool`.
- **Two-level cache (deferred)**: moka L1 only for now. L2 Redis is Phase 5.5 if multi-pod gateway emerges.

---

### Phase 6 — Tests

| Test | Type | Path |
|---|---|---|
| Namespace TOML round-trip + validator rejects bad slugs | unit | `tools/manifest.rs`, `tools/validator.rs` |
| `NamespaceFilter::to_sql_predicate` (Exact/Prefix/AnyOf) | unit | `tools/namespace.rs` |
| `top_n_capabilities_filtered` SQL with namespace + tags | sqlx integration | `vector_store/postgres.rs` |
| `SemanticCapabilityRouter::select` returns ≤K, respects filter, cache hit/miss | unit (in-mem store) | `tools/semantic_router.rs` |
| Rig `ToolProvider` adapter conformance | unit | `tools/semantic_router.rs` |
| Tower quota middleware rejects on over-limit | unit | `agent-gateway/src/mw/router_quota.rs` |
| `DynamicPromptCapability::invoke` reads DB row, renders, calls LLM mock | integration | `chains/dynamic_prompt.rs` |
| Dynamic prompt versioning: PUT bumps, GET ?version=N retrieves | gateway integration | `routes/admin_capabilities.rs` |
| `BulkCapabilityFactory::load_batch` with 1k specs (testcontainers) | integration | `tools/providers/capability_spec.rs` |
| LISTEN/NOTIFY hot-reload of one capability spec | integration | `tools/providers/capability_spec.rs` |
| End-to-end: 5k synthetic ERP specs → agent turn → only top-K passed to Anthropic mock | gateway integration | `routes/agent.rs` |

Run via `make test`. Add CI matrix entry: `cargo test -p agent-core --features local-embeddings`.

---

### Phase 7 — Documentation & migration

- Update [docs/arch.md](docs/arch.md) with the router diagram from §1.
- Add ADR `docs/adr/0004-semantic-capability-router-and-dynamic-prompts.md` documenting:
  - Top-K vs. all-tools trade-off.
  - Static (`capability.toml`) vs. dynamic (DB) prompts.
  - Hybrid Rust/WASM + prompt policy.
  - Why Rig's `ToolProvider` adapter (one Rust path, two surfaces).
- Backfill script `apps/backend/scripts/backfill_capability_namespaces.sql` — sets `namespace = ''` and `tags = '{}'` (the migration default already does this; script is a no-op safety net).
- Update [docs/project-instructions.md](docs/project-instructions.md): *"Every new capability MUST declare `namespace` and SHOULD declare `tags`."*
- New `docs/tasks/capability-pack.md`: how to author a capability spec row + WASM payload + prompt template (uses ERP as the worked example).

---

## 3. Sequencing & Dependencies

```
Phase 1 (namespaces + filters) ──► Phase 2 (SemanticCapabilityRouter + Rig adapter) ──► Phase 5 (obs)
                                              │
                                              ├──► Phase 3 (DynamicPromptCapability)
                                              │
                                              └──► Phase 4 (BulkCapabilityFactory + ERP)
                                                              │
                                                              └──► Phase 6 (tests) ──► Phase 7 (docs)
```

Phases 3 and 4 are independent and can be parallelised once Phase 2 is merged.

**Priority order:**
1. Merge Phase 1 (smallest blast radius).
2. Implement & merge `SemanticCapabilityRouter` with Rig `ToolProvider` integration.
3. Parallel: `DynamicPromptCapability` + `CapabilitySpecFactory`.
4. Full e2e test with 5k synthetic ERP specs before declaring v0.3.2 done.

---

## 4. Success Criteria

1. **Scale:** 10,000 capabilities registered; `capability_embeddings` holds 10k rows; cold start < 30 s; warm router p95 < 15 ms.
2. **Context budget:** LLM never receives > 4 KB of tool descriptions per turn (top-K=20, avg 200 chars/def).
3. **Extensibility:** Adding a new ERP rule = `INSERT` into `capability_specs` (namespace `erp.*`) + (optional) `dynamic_prompts` row. No Rust rebuild, no restart.
4. **Determinism preserved:** Money-moving primitives still implemented as WASM/Builtin providers, audited, schema-validated.
5. **Backward compatible:** Existing `capability.toml` files work unchanged; `namespace` and `tags` are optional.
6. **Rig-native:** `SemanticCapabilityRouter` is consumable via `rig::AgentBuilder::tools(adapter)` with no extra glue.
7. **Observable:** Per-turn trace shows selected capabilities, distances, cache hits, and invoke outcomes; OTel GenAI conventions emitted.

---

## 5. Out of Scope (deferred to v0.4)

- Multi-tenant capability *isolation* (`tenant_capability_grants` table).
- Capability *composition* DSL (declarative chains in YAML beyond `LlmChainConfig`).
- Cross-encoder re-ranking after vector top-K (small ONNX model).
- Auto-evaluation harness for prompt regressions (extend [apps/backend/evals](apps/backend/evals)).
- L2 Redis cache for multi-pod gateway deployments.
- `GraphCapabilityRouter` — orchestrator-style routing with explicit DAGs.
