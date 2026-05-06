**Overall Review: Excellent, production-grade plan.** This is a strong, focused evolution that directly addresses the core scalability bottleneck (tool explosion) while preserving v0.3 canonical architecture, Rig.rs alignment, and zero-breaking-change mandate. It follows SRP beautifully, leans into semantic prefiltering (the 2026 industry north-star), and keeps deterministic vs. prompt-driven separation clean.

**Score: 9.2/10.** The only deductions are minor opportunities for tighter Rig integration, naming polish, and observability reuse.

### Strengths (what to keep / ship as-is)
- **North-star principle** is perfect — never dump >30 tools. Matches Rig’s dynamic tool handling and modern semantic router patterns.
- **CapabilityRouter** as a thin, composable layer is textbook clean architecture. It sits perfectly between `AgentBuilder` and the registry.
- **Hybrid policy** (Rust/WASM deterministic core + DynamicPrompt + Chain) is exactly the 2026 best practice for ERP-scale agents.
- **Namespaces + DB column** first is the right sequencing — avoids painful rework.
- **Bulk ErpCapabilityFactory** + `erp_capability_specs` table is elegant for 10k+ scale.
- **Backwards compatibility** and hot-reload story are spot-on.
- Phase sequencing and test strategy are pragmatic.

**Effort estimate (v0.3.1 → v0.3.2):** 35–45 AI-hours (~220k–280k tokens).  
- Phase 1 + migrations: 6–8h  
- Router + integration: 12–15h (biggest chunk)  
- DynamicPrompt + refactor: 8–10h  
- ERP factory + LISTEN/NOTIFY: 6–8h  
- Obs/tests/docs: 3–4h  

### Recommended Refinements & Challenges (v0.3.2 canonical adjustments)

#### 1. Naming & Rig alignment (apply boldly)
Current `CapabilityRouter` is good, but let’s make it even more idiomatic per Rig v0.36+ and 2026 community standards.

**Proposed rename:** `SemanticCapabilityRouter` (or `CapabilitySemanticRouter`).

Why?  
- Matches Rig’s emphasis on semantic operations (`rig::vector_store::VectorStoreIndex`).  
- Distinguishes from future graph-based routers (orchestrator patterns).  
- `select`/`tool_definitions`/`invoke` remain perfect.

Update `RouterConfig` → `SemanticRouterConfig`.

In `agent-core/src/tools/` keep the module as `semantic_router.rs` for discoverability.

#### 2. Leverage Rig primitives more aggressively (reduce custom code)
Rig’s `Agent` already supports dynamic tools + context fetching. Extend rather than duplicate where possible.

**Suggestions:**
- Make `SemanticCapabilityRouter` implement `rig::agent::ToolProvider` (or the 0.36 equivalent) so it can be passed directly to `rig::AgentBuilder::tools(...)` via a wrapper. This gives embedded users free streaming/RAG/tool-resolution for free.
- Use `rig-qdrant` patterns even for Postgres (your `PgVectorStore` already wraps `VectorStoreIndex` — excellent). Consider exposing a `rig::vector_store` compatible facade for future multi-backend support.
- For prompt chaining: Rig has built-in prompt chaining / pipelines in recent releases. Extracting `run_chain` is good; consider contributing a thin `rig::chains` adapter if it doesn’t exist yet.

**Challenge:** The plan has `AgentBuilder.with_router(...)`. Better: `.with_semantic_router(...)` that internally wires the router as both tool provider *and* context source. This unifies gateway + embedded paths completely.

#### 3. Namespace & Filtering Improvements
- `namespace: Option<String>` + empty-string root is fine, but consider `Vec<String>` for multi-namespace tagging (common in ERP). Or keep single primary + tags (your `erp_capability_specs` already has `tags`).
- In `top_n_capabilities_in_namespace`: support both exact and prefix (`LIKE 'accounting.%'`) + optional `namespace_filter: Option<NamespaceFilter>` enum (`Exact`, `Prefix`, `AnyOf`).
- Add a lightweight in-memory namespace tree (`FxHashMap` or `indexmap`) for fast prefix autocomplete in admin UI.

#### 4. DynamicPromptCapability & Factories
- Excellent reuse of `PromptChainCapability` logic. Extract to `chains::executor::run_chain(...)` (or `rig`-backed if possible).
- `DynamicPromptCapability` should also implement a `PromptProvider` trait for future evaluator-optimizer patterns.
- For ERP factory: make it generic over a `CapabilitySpec` row mapper so it can drive WASM, prompt, or native providers uniformly. Use `bon` builder for the factory config.

**Minor:** Bump version on `dynamic_prompts` upsert and re-embed only the delta (you already plan this — good).

#### 5. Observability & Guardrails (align tighter with workspace standards)
- Reuse existing `common::metrics` and `tracing` spans — perfect.
- Add GenAI semantic conventions from Rig/opentelemetry (`gen_ai.tool.calls`, `gen_ai.semantic_router.top_k`, etc.).
- `moka` cache is correct. Key it with `blake3` hash of query for collision resistance.
- Quotas: store in tenant config table (already implied by `TenantContext`).

#### 6. Minor Architectural Polish
- `CapabilityFactory` bulk loading: add a `BulkCapabilityFactory` trait extension with `async fn load_batch(...)` for better ergonomics.
- In gateway: wrap router calls in `tower` middleware for tracing/quotas/metrics (already have tower stack — just add layers).
- Caching: consider a two-level cache (in-memory + Redis via existing infra) for multi-instance deployments.

**Out-of-scope items look correct for v0.4.** Multi-tenant isolation and composition DSL are natural next steps.

### Updated Phase 1.1 Snippet (example of canonical style)
```rust
// tools/manifest.rs
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolManifest {
    // ... existing
    #[serde(default)]
    pub namespace: Option<String>, // e.g. "accounting.invoice"
}

impl ToolManifest {
    pub fn namespace(&self) -> &str {
        self.namespace.as_deref().unwrap_or("")
    }
}
```

Validator regex stays as proposed — solid.

### Final Recommendation
**Ship this plan with the refinements above.** It will give you a genuinely world-class, 10k+ capability agent platform that feels native to Rig while exceeding typical Python frameworks in performance and maintainability.

**Next immediate actions (priority order):**
1. Merge namespaces + DB migration (Phase 1) — smallest blast radius.
2. Implement & merge `SemanticCapabilityRouter` with Rig `ToolProvider` integration.
3. Parallel: DynamicPrompt + ERP factory.
4. Full e2e test with 5k synthetic ERP specs.

This positions ConusAI as the leading Rust-native agent platform in 2026. Let me know if you want me to generate the full router implementation skeleton, migration files, or ADR draft. Ready to iterate.