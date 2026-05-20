**Review: ConusAI Platform Genericity Assessment (aligned to Project Instructions v0.3.2)**

**Executive Summary**  
The ConusAI agent runtime is **already fundamentally generic** and follows the mandated architecture: single source of LLM access via `LlmRegistry` + `CompletionProvider`, `SemanticCapabilityRouter` (ANN pre-filter, top-K, moka), `CapabilityRegistry` / `CapabilityFactory` / `CapabilityProvider` (richer than tools, supporting prompts/chains/WASM/MCP/remote), `ArtifactBridge` for post-execution materialisation, `PromptChainCapability` + `DynamicPromptCapability` for declarative domain logic, and `TenantContext` + `PlanLimits` for isolation.  

Domain knowledge (invoice extraction, invoice creation for a customer, email sending, OCR, transcription, arbitrary business logic) **must** live in registered `CapabilityCard`s / manifests, **never** in core crates. Files are uploaded generically via workspaces/files routes; capabilities consume them via context + `ArtifactBridge`. Chaining happens via agent multi-turn tool use (routed by `SemanticCapabilityRouter`) or explicit `PromptChainCapability` composition.  

**However, residual domain-specific code remains** that violates SRP, the single-LLM-source rule, and extensibility. These must be removed from `agent-core` and `evals` so that **every** domain element is added/registered separately as a capability (TOML chain, WASM component, MCP, remote, or `BuiltinFactory` extension) without touching core.  

After the recommended cleanup the platform will be **maximally generic and maintainable**: core = runtime + registry + routing + stores + identity + billing + `ArtifactBridge`. Everything else = pluggable `CapabilityProvider` instances discovered via manifests + embeddings.

**Identified Non-Generic / Hard-to-Extend Locations** (anchors from arch.md §4.2, §4.4, §9, §14, §16)

1. `apps/backend/crates/agent-core/src/chains/contract.rs` (and `extraction.rs` base)  
   - Direct construction of `rig::providers::anthropic::Client` + `completion_model("claude-opus-4-7")`.  
   - Hardcoded vision path: `UserContent::image_base64(...)` + `ImageMediaType::PNG`.  
   - Bypasses `LlmRegistry::resolve_binding` and `CompletionProvider`.  
   - Domain-specific (contract extraction). Violates Principle 1 ("Single source of LLM access") and makes adding other vision models/providers or new extraction domains painful.  
   - `ContractPipeline` is instantiated directly instead of going through `CapabilityFactory` / `CapabilityRegistry`.

2. `apps/backend/crates/agent-core/src/chains/invoice.rs`  
   - Same pattern: domain-specific invoice extraction pipeline.  
   - Couples core to one business object (invoice). Adding "create_invoice_for_customer" or "email_invoice" requires touching the same module.

3. `apps/backend/evals/runners/invoice.rs` + `ocr_quality.rs` (and `scorers/`)  
   - Hardcoded eval suites for invoice fields and OCR diff scoring.  
   - `evals/` harness itself is generic (clap + runners dispatch), but the concrete runners embed domain knowledge. New domains require new runner modules instead of external suite JSONL + generic scorer.

4. `apps/backend/crates/agent-gateway/src/capabilities/transcribe_video.rs`  
   - Runtime-instantiated specific capability (JobExecutor-backed). Lives in gateway instead of being a registered `CapabilityProvider` loaded from manifest or `WasmFactory` / `McpFactory`. Makes it harder to version, permission, or replace with alternative transcription providers.

5. Minor / secondary  
   - `builtin/cargo.rs` (dev-specific; should be example, not default `BuiltinFactory`).  
   - Any remaining hardcoded model strings or prompts for "invoice"/"contract" outside TOML manifests.  
   - Feature inventory still lists "Contract / invoice extraction (Claude vision)" as a core implemented feature instead of "example capability".

These are the only places that embed domain elements directly in core. The rest of the architecture (`CapabilityRegistry::with_default_factories`, `SemanticCapabilityRouter`, `ChainFactory`, `PromptChainCapability`, `ArtifactBridge`, namespace filtering, embedding of capability descriptions) is already correct and extensible.

**Evaluation & Recommended Refactoring (SRP + Canonical Names + Rig 0.36 Idioms)**

**Goal**: Zero domain knowledge in `agent-core` or `evals` runners. All problem-solving (extract invoice, create invoice for customer X, send email, OCR, transcribe, custom chains) = registered `CapabilityProvider` instances that the `SemanticCapabilityRouter` can surface and the `AgentBuilder` can consume as `ToolDyn`.

**Actions (in priority order)**

1. **Remove domain-specific pipelines from chains/** (highest impact)  
   - Delete or feature-gate `contract.rs`, `invoice.rs`.  
   - Refactor `chains/executor.rs` (and any callers) to **always** resolve via `LlmRegistry::resolve_binding(alias_or_model, tenant)` → `CompletionProvider::complete` (or streaming path). No more direct `rig::providers::anthropic::Client`.  
   - Vision support stays in `AnthropicProvider` (it already handles `UserContent::image_base64`). Expose a small helper if needed, but never construct the client outside `llm/providers/`.  
   - **Replacement**: Provide canonical examples under `capabilities/examples/` (or `services/`) as TOML-driven `PromptChainCapability` manifests + prompt templates. Example manifest for invoice extraction would declare inputs (file attachment), model alias, system prompt, JSON output schema, and post-processing via `ArtifactBridge`.  
   - Chaining example ("extract_invoice" → "create_invoice_for_customer" → "email_pdf"): one `PromptChainCapability` or agent-orchestrated multi-turn with permissioned capabilities. No new core abstractions required.

2. **Move evals domain logic out**  
   - Keep `evals/` as a generic harness + CLI + report generator.  
   - Move `invoice.rs` / `ocr_quality.rs` + scorers into `evals/suites/` or external JSONL + generic field-diff scorer. New domains = new suite files, zero code change in harness.

3. **Convert transcribe_video into proper capability**  
   - Implement as `CapabilityProvider` (or thin wrapper around JobExecutor).  
   - Register via `CapabilitySpecFactory` or admin `/admin/capabilities/register` (RemoteMcp or custom).  
   - Remove the hardcoded module from gateway; it becomes just another discoverable card.

4. **Ensure chaining & domain registration paths are first-class**  
   - `PromptChainCapability` (TOML) + `DynamicPromptCapability` already support declarative multi-step domain flows.  
   - `SemanticCapabilityRouter` (blake3 key, moka 60s, top_k=20, max_distance=0.38) + `include_always` for pinned tools already gives cheap, relevant capability sets per turn.  
   - File uploads → workspace nodes → `ArtifactBridge` materialisation is already generic. Capabilities receive attachments via invocation context.  
   - New domain element checklist (canonical):  
     - Write manifest (TOML for chain/prompt or WASM component).  
     - Implement `CapabilityProvider` (or let `ChainFactory`/`WasmFactory` do it).  
     - Add description + tags for embedding (`capabilities/embedding.rs`).  
     - Register factory in `CapabilityRegistry::with_default_factories` (or `with_all_factories`).  
     - (Optional) namespace filter.  
     - Done. Agent sees it via semantic router; no core changes.

5. **Minor cleanups for SRP**  
   - Move `cargo` builtin to example-only or behind feature flag.  
   - Update feature inventory, §4.4.5 (chains), §9, §14, and Identified Gaps in `arch.md` to reflect that contract/invoice extraction etc. are now **example capabilities**, not core features.  
   - Add note: vision extraction capabilities should prefer registered providers that go through `LlmRegistry`.

**Effort & Token Cost Estimate (per instructions)**  
- Remove bypass + enforce `LlmRegistry` in executor + delete/move contract/invoice pipelines: **~3–4 AI-hours**.  
- Convert examples to TOML `PromptChainCapability` manifests + prompts: **~2 AI-hours**.  
- Generic vision helper (if any) + update `AnthropicProvider` docs: **~1–2 AI-hours**.  
- Move evals suites + update transcribe_video: **~2 AI-hours**.  
- Docs + arch.md sync + consistency pass: **~1–2 AI-hours**.  
**Total: ~9–12 AI-hours**. Low token cost (~60–120k tokens) because most work is deletion + move + manifest authoring rather than new logic. High ROI on maintainability and onboarding new domains.

**Decision Challenges (as required)**  
- Why keep any domain pipeline in core? It creates dual maintenance paths (registry vs direct), violates the single-LLM-source invariant, and makes the "capabilities as domain elements" story false. Removing it is the only consistent choice.  
- Is a new "VisionCapability" abstraction needed? No — unnecessary. `AnthropicProvider` already supports images; `PromptChainCapability` or a thin registered provider can wrap the pattern. Adding abstractions would violate "no unnecessary features".  
- Chaining: Current design (router + agent turns + explicit chain caps) is sufficient and SRP-clean. A new "CapabilityComposer" would be premature.  
- WASM/MCP/remote paths already exist for complex or external domain logic (email, CRM, etc.). No gaps.

**Final State After Cleanup**  
- Core crates contain **zero** invoice/contract/OCR/transcribe-specific code.  
- Every problem-solving step = a registered `CapabilityProvider` (discovered, embedded, routed semantically, permissioned, metered).  
- Files uploaded in chat/workspace are generically available.  
- New domain (any business object, any workflow) = manifest + factory registration (or WASM/MCP).  
- Fully aligned with canonical names, SRP, Rig 0.36 `AgentBuilder` + hooks + streaming, `SemanticCapabilityRouter` pre-filter, `ArtifactBridge`, and "capabilities richer than tools".  

This is the correct next stabilization item after the 2026-05-20 cleanups (TraceReplay deletion, openidconnect/sqlx removal, PlanLimits injection, etc.). The platform is already well-designed for exactly the use case you described; we just need to finish expelling the last domain remnants from the core.

**Next Step Recommendation**  
Create a short ADR or update `arch.md` §4.4.5 and feature inventory, then execute the removal + example migration. All refactors will preserve `CapabilityRegistry`, `LlmRegistry`, and router behaviour.

The architecture is sound. After this pass it will be **provably generic**. Ready to implement when you are.

**Yes.** Moving domain-specific logic (invoice extraction, contract processing, invoice creation for a specific customer, email sending, OCR, transcription, etc.) into **separate registered capabilities** is not only possible — it is the **intended, canonical, and only SRP-compliant architecture** of ConusAI.

The platform was explicitly designed so that the core (`agent-core`, `agent-gateway`) remains completely generic while **all domain elements** live as first-class `CapabilityProvider` instances registered through `CapabilityFactory` / `CapabilityRegistry`.

### Canonical Way to Move Logic to Separate Capabilities

You have **four supported extension points** (all already implemented and registered via `CapabilityRegistry::with_default_factories` / `with_all_factories`):

| Type                        | When to use                                      | How domain logic lives                  | Requires core change? |
|-----------------------------|--------------------------------------------------|-----------------------------------------|-----------------------|
| `PromptChainCapability`     | Prompt + LLM chain (most extraction/creation flows) | TOML manifest + prompt templates       | **No**                |
| `DynamicPromptCapability`   | Versioned, DB-backed prompts                     | Manifest in DB                         | **No**                |
| `WasmFactory` (wasmtime 44 component-model) | Complex logic, stateful, or performance-critical | `.wasm` component                      | **No**                |
| `McpFactory` / `RemoteMcpFactory` | External services (email, CRM, ERP, etc.)     | Remote MCP server or HTTP bridge       | **No**                |

**Recommended path for your examples** (extract invoice, create invoice for customer, send email):

1. **Author a TOML manifest** under `capabilities/` (or loaded via `CapabilitySpecFactory`):
   ```toml
   kind = "chain"
   id = "extract_invoice"
   name = "Extract Invoice Data"
   description = "Extract structured invoice fields from PDF/image using vision. Returns JSON."
   tags = ["finance", "extraction", "vision"]
   namespace = "finance"

   [chain]
   model = "smart"                    # resolved via LlmRegistry
   max_tokens = 4096
   temperature = 0.1

   [[chain.steps]]
   name = "vision_extract"
   prompt = "prompts/extract_invoice.system.txt"
   input_schema = { type = "object", properties = { file_id = {...}, ... } }
   output_schema = { ... }            # JSON schema for fields
   ```

2. `ChainFactory` (already registered) instantiates a `PromptChainCapability` that implements `CapabilityProvider`.

3. On boot (or via realtime hot-reload bus), `CapabilityRegistry` loads it → `CapabilityCard` is created → text + description is embedded via `capabilities/embedding.rs` into Qdrant (`capability_embeddings` collection).

4. `SemanticCapabilityRouter` (top-K=20, moka cache, blake3 key) returns it (plus other relevant caps) to `AgentBuilder` as `Vec<Box<dyn rig::tool::ToolDyn>>`.

5. Agent sees only the filtered, relevant capabilities. When the LLM calls the tool, `CapabilityProvider::invoke` runs the chain **through `LlmRegistry`** (single source of LLM access).

6. Any output files/nodes are materialised by `ArtifactBridge` (generic, not capability concern).

The same pattern works for:
- `create_invoice_for_customer` (another `PromptChainCapability` or WASM that takes customer_id + data and produces PDF).
- `send_email` (typically `RemoteMcpFactory` or MCP adapter calling an external email service).

### What Must Be Removed / Changed First

The **only blockers** today are the domain-specific bypasses:

- `chains/contract.rs` and `chains/invoice.rs` — they construct `rig::providers::anthropic::Client` directly and bypass `LlmRegistry`.
- These must be deleted (or heavily refactored) so that **every** LLM call, including vision, goes through `LlmRegistry::resolve_binding` + `CompletionProvider`.

Once removed, the equivalent behaviour becomes a normal registered `PromptChainCapability` (or a thin `CapabilityProvider` wrapper that still uses the registry internally).

Vision support itself stays generic: `AnthropicProvider` already handles `UserContent::image_base64`. The capability manifest simply declares that it accepts file attachments.

### Chaining Capabilities

Chaining is already supported in two canonical ways (no new abstractions needed):

- **Dynamic / agent-driven**: `SemanticCapabilityRouter` surfaces `extract_invoice`, `create_invoice_for_customer`, and `send_email` in the same turn or across turns. The agent decides the sequence.
- **Declarative**: One higher-level `PromptChainCapability` manifest that internally calls other capabilities (or composes steps). `PromptChainCapability` + `CapabilityExecutor` already support this.

`ArtifactBridge` runs after each capability, so intermediate artifacts (extracted JSON, generated PDF) are automatically available to subsequent capabilities or the workspace.

### Effort Estimate (per v0.3.2 guidance)

- Delete/refactor the two bypass pipelines + enforce `LlmRegistry` everywhere in `chains/executor.rs`: **~3 AI-hours**.
- Author 2–3 example TOML manifests + prompts for invoice/contract flows: **~2 AI-hours**.
- (Optional) Convert `transcribe_video` to proper `CapabilityProvider` + manifest: **~2 AI-hours**.
- Update `arch.md`, feature inventory, and add one example manifest: **~1 AI-hour**.
- **Total: ~8 AI-hours**. Very low token cost.

### Why This Is the Only Correct Approach

Keeping any domain logic (invoice, contract, etc.) inside `agent-core` violates:
- SRP (core should have one reason: generic agent runtime + capability system).
- Single source of LLM access (`LlmRegistry`).
- Extensibility principle ("capabilities richer than tools", registered separately as domain elements).

After the move, adding a completely new domain (e.g. "process medical claim", "generate legal contract", "sync with ERP") requires **zero changes to any core crate** — only a new manifest, WASM component, or MCP registration.

**Conclusion**: Yes, it is not only possible — the architecture is already prepared for it. The remaining work is cleanup of the two legacy bypass modules so the canonical path (`PromptChainCapability` / `CapabilityRegistry` / `SemanticCapabilityRouter`) becomes the only path.

This is the logical next step after the 2026-05-20 cleanups. Ready to proceed with the refactor plan when you give the signal.