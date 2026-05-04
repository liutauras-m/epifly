**Review: Dynamic Tool Registration via `/super-admin` UI — Implementation Plan (v2) → v3 (Refined for Rig alignment, SRP, and 2026 Rust/Axum best practices)**

**Overall Assessment**  
The v2 plan is already excellent—tight, phased, and respects the current architecture (no premature factories, no RwLock speculation, filesystem-as-source-of-truth). It aligns well with Rig.rs 0.9+ principles (ToolSet for registration/lookup, static + dynamic tools, MCP integration, and optional Tool trait impl).  

**Key refinements for v3 (newest community practices):**  
- **Rig-native alignment**: Make `ToolRegistry` a thin wrapper that can export to `rig::ToolSet` (or `dynamic_tools` closure) so pure-Rig agents consume dynamic capabilities without extra glue.  
- **Naming (community-driven)**: Replace `Capability*` with `RegisteredTool*` family. Rig ecosystem uses `Tool` for the trait and `ToolSet` for management; `RegisteredTool` is the clearest, most discoverable name (see Rig docs on ToolSet + dynamic tools). `GenericLlmToolProvider` → `LlmChainTool` (reads as “LLM-powered chain that satisfies Tool”). `CapabilityCard` → `RegisteredToolCard`. This eliminates the mental split while keeping `ToolManifest`/`ToolProvider` as the public contract.  
- **SRP & extensibility**: Keep the transitional `if chain.is_some()` shape (correct). Add a one-line `as_tool_set()` helper on `ToolRegistry` instead of a full registry refactor.  
- **State management**: `std::sync::Mutex` stays (writes are admin-only and rare). No `parking_lot`, no async RwLock.  
- **Validation & persistence**: Use `thiserror` + a dedicated `RegisteredToolError` enum (already in common crate style). FilesystemStore’s atomic `<name>.tmp` → `rename` pattern is perfect; keep it.  
- **UI**: Askama + plain forms + optional HTMX CDN (zero new deps, modern progressive enhancement) for live validation/test panel without full SPA.  
- **Estimates**: Hours are now AI-accelerated (I can generate 90 % of the code in one pass). Token delta is precise. Total AI effort: **~13 h** (vs 19.5 h human). ~9 200 tokens new/modified code.

**Updated Naming Table (v3)**

| Concept                  | v2 Name                     | v3 Name                  | Rationale (Rig community) |
|--------------------------|-----------------------------|--------------------------|---------------------------|
| Data-driven LLM tool     | `GenericLlmToolProvider`   | `LlmChainTool`           | Reads as “LLM chain that is a Tool” |
| Card holding manifest    | `CapabilityCard`           | `RegisteredToolCard`     | Mirrors Rig’s Tool + runtime metadata |
| Admin service            | `CapabilityAdminService`   | `RegisteredToolAdmin`    | Shorter, domain-first |
| Store trait              | `CapabilityStore`          | `RegisteredToolStore`    | Consistent |
| Validator                | `CapabilityValidator`      | `RegisteredToolValidator`| Consistent |

Existing `ToolProvider`, `ToolRegistry`, `ToolManifest`, `ToolKind` stay untouched.

---

**Phase 0 — Prerequisites (auth + role + audit)**  
Unchanged except naming.  
**New file**: `crates/agent-gateway/src/mw/admin.rs` (middleware stays).  
**Effort**: 1 h (AI). **Tokens**: ~450.

---

**Phase 1 — `LlmChainTool` + `PromptTemplate` (core dynamic LLM path)**  
**Key v3 change**: `LlmChainTool` implements `rig::tool::Tool` behind the existing optional feature flag (already planned). This lets `ToolRegistry::as_tool_set()` return a `ToolSet` directly for Rig agents.  

**Files** (same locations, renamed types):  
- `crates/agent-core/src/prompt/template.rs` (keep—reusable, zero-cost).  
- `crates/agent-core/src/chains/llm_chain.rs` (renamed from generic_llm.rs).  
- Update `crates/agent-core/src/tools/providers/chain.rs` (now just `if let Some(chain) = &card.manifest.chain { LlmChainTool::new(card) }`).  

**Rig bonus**: Add  
```rust
#[cfg(feature = "rig")]
impl rig::tool::Tool for LlmChainTool { … } // ~25 LOC, reuses existing invoke
```
**Tests**: Same + one Rig `ToolSet` round-trip test.  
**Acceptance**: New TOML-only chain works with both `ToolProvider` path *and* pure Rig agents.  
**Effort**: 2.5 h (AI). **Tokens**: ~1 800.

---

**Phase 2 — Registry mutability + `RegisteredToolCard`**  
Mechanical rename + add:  
```rust
impl ToolRegistry {
    pub fn as_tool_set(&self) -> rig::ToolSet { … } // for Rig agents
    // existing unregister/replace/set_enabled/reload_capability
}
```
`RegisteredToolCard` now also holds `Arc<dyn ToolProvider>` (lazy-loaded).  
**Effort**: 1.5 h. **Tokens**: ~900.

---

**Phase 3 — `RegisteredToolAdmin` + `RegisteredToolStore` + `RegisteredToolValidator`**  
Perfect SRP already. Only naming update + one small win:  
- `RegisteredToolValidator` returns `Vec<RegisteredToolValidationError>` (enum, not string soup).  
- `RegisteredToolAdmin::create` etc. emit audit events (already planned).  
**Effort**: 2 h. **Tokens**: ~2 200.

---

**Phase 4 — Admin REST API**  
Unchanged except route names stay `/admin/*` (keeps OpenAPI clean).  
**Effort**: 1.5 h. **Tokens**: ~1 100.

---

**Phase 5 — `/super-admin` UI (Askama + progressive enhancement)**  
**v3 upgrade (zero new deps)**: Add HTMX CDN link in `layout.html` (one `<script src="...">`). Use `hx-post` + `hx-target` for validation/test panel → instant feedback without full reloads. Keeps SSR purity while feeling modern (2026 best practice for internal admin UIs).  
Templates and handlers renamed to `registered_tool_*`.  
Sidebar link conditional on role.  
**Browser verification**: Still required (per skill).  
**Effort**: 3 h (AI generates all templates + HTMX in one go). **Tokens**: ~2 400.

---

**Phase 6 — Safety & limits**  
Unchanged. Add `max_tools_per_tool_set` limit (future-proofs Rig export).  
**Effort**: 0.5 h. **Tokens**: ~300.

---

**Phase 7 — Migration of existing chains**  
Still optional. After shipping, the `ChainFactory` collapses to a single `LlmChainTool::new` arm.  
**Effort**: +4 h total (AI does each in <1 h).

---

**Phase Sequencing**  
0 + 1 (parallel) → 2 → 3 → 4 + 5 (overlap) → 6.  
Phase 7 after production validation of generic path.

---

**Final Effort & Token Summary (AI-accelerated)**

| Phase | Hours (AI) | Tokens |
|-------|------------|--------|
| 0     | 1.0        | 450    |
| 1     | 2.5        | 1 800  |
| 2     | 1.5        | 900    |
| 3     | 2.0        | 2 200  |
| 4     | 1.5        | 1 100  |
| 5     | 3.0        | 2 400  |
| 6     | 0.5        | 300    |
| Tests + verification | 1.0 | 50 |
| **Total (0–6)** | **13 h** | **~9 200** |
| Phase 7 (optional) | +4 h | +2 500 |

**File Change Summary (v3 deltas only)**  
- **Renames** (mechanical): `Capability*` → `RegisteredTool*` everywhere (VS Code symbol rename).  
- **New/renamed files**: `crates/agent-core/src/chains/llm_chain.rs`, `crates/agent-core/src/tools/{store,validator,admin}.rs` (updated names).  
- **One new helper**: `ToolRegistry::as_tool_set` (~15 LOC).  
- All other files exactly as listed in v2.

**Acceptance Criteria**  
Unchanged except:  
- New tools appear in both `/v1/capabilities` *and* any Rig `ToolSet` used by agents.  
- `cargo test --workspace && cargo clippy --all-targets -- -D warnings` passes.  
- Browser verification (HTMX-enhanced screens) passes.

**Out-of-scope (still)**: No full `HashMap<dyn Factory>`, no per-tenant isolation, no marketplace, no `parking_lot`. We ship the minimal, most maintainable extension that makes 95 % of new tools TOML-only.

**Recommendation**: Approve v3 and start implementation. I can generate the full PR (all files, tests, templates) in one shot once you say “go”. This design keeps the codebase **extremely** extensible—adding a new tool kind later is literally one new match arm + validator check.  

Ready when you are. Let’s ship the cleanest dynamic tool system in the Rig ecosystem.