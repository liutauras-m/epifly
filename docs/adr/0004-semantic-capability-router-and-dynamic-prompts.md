# ADR 0004 — Semantic Capability Router & Dynamic Prompts

**Date:** 2026-05-07  
**Status:** Accepted  
**Deciders:** ConusAI platform team

---

## Context

As the platform scales to support enterprise ERP integrations, the capability registry can contain thousands of tools (e.g. one per ERP workflow). Sending all tool definitions to an LLM context is:

1. **Infeasible** — Anthropic context windows cap at ~200K tokens; 5K tool schemas easily exceed that.
2. **Expensive** — More tokens = higher cost per turn.
3. **Degrading** — LLMs perform worse with irrelevant tools in context ("lost in the middle").

Additionally, many enterprise clients need to customise agent behaviour per-customer without a deploy cycle (e.g. swap a prompt template for an invoice extraction workflow).

## Decision

### 1. Semantic Capability Router

Introduce `SemanticCapabilityRouter` as the single entry point for resolving capabilities per agent turn:

- Embed the user query with the same model used for capability indexing.
- ANN search `capability_embeddings` table using pgvector + DiskANN.
- Apply namespace/tag filters and cosine distance threshold (≤ 0.65 by default).
- Return top-K (≤ 50) providers; pass only their tool definitions to the LLM.
- Cache results with a 60s moka TTL keyed by blake3(tenant_id + query + config).

This replaces the previous "send all enabled tools" approach in `agent.rs`.

### 2. Namespace & Tag Partitioning

Extend `ToolManifest` with `namespace: Option<String>` (dot-separated slug) and `tags: Vec<String>`. This allows:
- Per-tenant namespace routing (`erp.acme.*` only for Acme Corp).
- Tag-based pre-filters (`["finance", "gl"]` only for accounting queries).
- Hierarchical admin autocomplete via `ToolRegistry::namespace_children(prefix)`.

### 3. Dynamic Prompts

`ToolKind::DynamicPrompt` capabilities load their `LlmChainConfig` from a `dynamic_prompts` Postgres table at runtime. Admins can:
- Push a new prompt version via the REST API without redeploying.
- Roll back by pinning to an older version.
- A/B test by creating two capabilities pointing to different versions.

### 4. Bulk ERP Factory

`CapabilitySpecFactory` (implements `BulkCapabilityFactory`) streams from `capability_specs` at boot, generating `CapabilityProvider` + embeddings in 256-row batches. Hot-reload via Postgres `LISTEN capability_specs_changed` means changes propagate in < 1s without restart. The factory is domain-neutral; ERP is the first vertical, partitioned by `namespace = 'erp.*'`.

### 5. OTel GenAI Metrics

Follow OpenTelemetry GenAI semantic conventions for router instrumentation. Key metrics: `gen_ai.semantic_router.cache_hit`, `gen_ai.semantic_router.top_k`, `gen_ai.semantic_router.distance`, `capability_router_select_seconds`.

### 6. Tower Quota Middleware

`RouterQuotaLayer` on `/v1/agent/completions` enforces hard caps on tools-per-turn and invokes-per-turn, protecting against runaway agent loops.

## Consequences

### Positive

- Agents with 5K+ tools remain fast and cheap — only top-K reach the LLM.
- Dynamic prompts remove the deploy friction for prompt iteration.
- ERP integrations can be configured in the database without Rust code changes.
- Namespace partitioning enables multi-tenant capability isolation.
- OTel metrics give operational visibility into router health.

### Negative / Trade-offs

- Cache invalidation is TTL-based (60s); stale results possible within the window. Call `router.invalidate_all()` for immediate invalidation (e.g. after bulk import).
- ANN search quality depends on embedding model consistency. Re-embedding is required if the model changes.
- `capability_specs` is a single table; multi-schema integrations may need tenant-scoped namespacing (handled via `namespace` column).

## Alternatives Considered

| Alternative | Reason rejected |
|---|---|
| Keyword search (full-text) | Lower recall for paraphrase queries; not semantic |
| Qdrant vector DB | Eliminates the Postgres-only constraint adopted in v0.3.1 |
| Rule-based routing (config file) | Brittle; doesn't scale to 5K+ tools |
| Streaming all tools | Context window and cost constraints — see above |
