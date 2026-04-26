# ConusAI Platform — `Capability*` → `Tool*` Refactor Plan v0.3

**Goal:** Rename `Capability*` → `Tool*` across the workspace and replace the imperative `match` in `tool_executor.rs` with a `ToolProvider` trait + registry. Pure refactor — no API/behaviour changes, no schema changes, no new features.

**Why:** Rig 0.9+ and the MCP ecosystem call these "tools". The current name "capability" is from an earlier draft and is now inconsistent with both the public-facing tool definitions (`ToolDef`, `tool_definitions()`, the UI's `appendToolCard`/`finalizeToolCard`) and the surrounding ecosystem.

**Status:** **Not started.** This document is the implementation contract; no code has moved yet.

**Scope guardrails — these MUST NOT regress:**

- Public HTTP routes: `/v1/capabilities`, `/v1/capabilities/search`, `/v1/agent/completions`, `/v1/chat/completions`, `/mcp`, `/v1/files`, `/v1/threads/*`, `/v1/workspaces/*`, `/v1/audit`. Path strings stay as `capabilities`.
- All `capabilities/*/capability.yaml` manifest files stay byte-identical.
- Workspace/folder system ([qdrant_workspace_store.rs](../crates/agent-core/src/memory/qdrant_workspace_store.rs)), audit log ([qdrant_audit.rs](../crates/agent-core/src/memory/qdrant_audit.rs)), thread store, content_text indexing, and the ~5-round agent loop in [agent.rs](../crates/agent-gateway/src/routes/agent.rs) all keep current behaviour.
- `cargo test --workspace` still passes 30 tests (8 `agent-core` + 22 `common`); no new tests required, but old tests must keep passing.

---

## Current state — what actually exists today

These are the inputs to the refactor. Verified 2026-04-26.

### Capability-side files (under `crates/agent-core/src/capabilities/`)

| File | Lines | Capability* symbol count | Contents |
|---|---:|---:|---|
| [`mod.rs`](../crates/agent-core/src/capabilities/mod.rs) | 9 | 0 | Pub mods only |
| [`provider.rs`](../crates/agent-core/src/capabilities/provider.rs) | 10 | 1 | `AgentCapability` async trait — currently **unused by `tool_executor.rs`** (the match dispatches by `kind`, not via the trait). Defines `name`, `description`, `tool_names`, `invoke`. |
| [`manifest.rs`](../crates/agent-core/src/capabilities/manifest.rs) | 58 | 7 | `CapabilityManifest`, `CapabilityKind { Mcp, Wasm, Pipeline, Docker, Native }`, `ToolDef { name, description, input_schema }`. `from_yaml`, `from_file`, `embedding_text`. |
| [`card.rs`](../crates/agent-core/src/capabilities/card.rs) | 21 | 5 | `CapabilityCard { id (UUID), manifest, source_path, embedding_id }`. |
| [`registry.rs`](../crates/agent-core/src/capabilities/registry.rs) | 129 | 21 | `CapabilityRegistry`: `HashMap<String, CapabilityCard>`. `register`, `get`, `search_by_tag`, `all`, `len`, `is_empty`, `load_from_dir`. |
| [`discovery.rs`](../crates/agent-core/src/capabilities/discovery.rs) | 31 | 5 | `CapabilityDiscovery::from_env()` reads `CONUSAI_CAPABILITIES_DIR`; `discover()` returns a populated `CapabilityRegistry`. |
| [`embedding.rs`](../crates/agent-core/src/capabilities/embedding.rs) | 11 | 2 | `ToolEmbedding::describe(card)` → `manifest.embedding_text()`. |
| [`mcp_adapter.rs`](../crates/agent-core/src/capabilities/mcp_adapter.rs) | 65 | 0 | `McpAdapter` — JSON-RPC 2.0 HTTP client. No `Capability*` symbols (already provider-style). |
| [`tool_executor.rs`](../crates/agent-core/src/capabilities/tool_executor.rs) | 317 | 12 | The big `match` — see below. Plus `tool_definitions(card) -> Vec<Value>` for Anthropic format. |
| [`wasm_loader.rs`](../crates/agent-core/src/capabilities/wasm_loader.rs) | 107 | 11 | `WasmCapabilityLoader` (wraps `wasmtime::Engine`). `load(card)`, `invoke_i32`, `invoke_tool`. |

**Total in this folder:** 758 lines, 64 `Capability*` occurrences.

### `tool_executor.rs` dispatch — the actual match (lines 65-272)

The match has **8 distinct arms**, not 3 as the v0.2 plan implied:

```rust
match (card.manifest.name.as_str(), tool_name) {
    ("contract-processing", "extract_contract")   => ContractPipeline::extract_from_document_path(...)
    ("contract-processing", "summarise_contract") => ContractPipeline::summarise(...)
    ("invoice-processing",  "extract_invoice")    => InvoicePipeline::extract_from_image_path(...)
    ("invoice-processing",  "validate_invoice")   => InvoicePipeline::validate(...)
    ("ocr-service",         "extract_text")       => InvoicePipeline (vision OCR mode)
    ("native-tools",        "read_file")          => fs_tools::read_file(workspace_root, input)
    ("native-tools",        "write_file")         => fs_tools::write_file(workspace_root, input)
    ("native-tools",        "run_cargo")          => cargo_tool::run_cargo(workspace_root, input)
    (_, _) if kind == Mcp     => McpAdapter::new(endpoint).call_tool(tool, input)
    (_, _) if kind == Wasm    => WasmCapabilityLoader::new().invoke_tool(card, tool, input)
    (_, _) if kind == Docker  => bail!("reserved")
    _                          => bail!("No executor registered for ...")
}
```

The first 8 arms hardcode `(capability_name, tool_name)` pairs. Replacing this with a trait is the central refactor.

### Files outside `capabilities/` that touch `Capability*`

| File | Lines | Capability* count | Why it touches them |
|---|---:|---:|---|
| [`crates/agent-core/src/lib.rs`](../crates/agent-core/src/lib.rs) | 18 | 2 | Re-exports `CapabilityDiscovery`, `CapabilityRegistry` |
| [`crates/agent-core/src/agent/runtime.rs`](../crates/agent-core/src/agent/runtime.rs) | — | 5 | `AgentRuntime` holds a `CapabilityRegistry` |
| [`crates/agent-core/src/tools/native_capability.rs`](../crates/agent-core/src/tools/native_capability.rs) | 77 | 7 | `native_capability_card()` returns a `CapabilityCard` with `kind: CapabilityKind::Native`. Imports `CapabilityKind`, `CapabilityManifest`, `ToolDef`. |
| [`crates/agent-core/src/pipelines/invoice.rs`](../crates/agent-core/src/pipelines/invoice.rs) | 191 | 4 | Doc comments + Card construction in tests |
| [`crates/agent-core/src/pipelines/contract.rs`](../crates/agent-core/src/pipelines/contract.rs) | 172 | 4 | Same as invoice |
| [`crates/agent-gateway/src/state.rs`](../crates/agent-gateway/src/state.rs) | 123 | 3 | `Mutex<CapabilityRegistry>` in `AppState` |
| [`crates/agent-gateway/src/routes/mod.rs`](../crates/agent-gateway/src/routes/mod.rs) | 73 | 1 | Mounts `/v1/capabilities` (route path — keep) |
| [`crates/agent-gateway/src/routes/agent.rs`](../crates/agent-gateway/src/routes/agent.rs) | 865 | 5 | Imports `CapabilityExecutor`; uses `CapabilityCard` + `CapabilityExecutor::tool_definitions` + `CapabilityExecutor::invoke` |
| [`crates/agent-gateway/src/routes/mcp.rs`](../crates/agent-gateway/src/routes/mcp.rs) | 132 | 4 | Same as `agent.rs` for `tools/list` and `tools/call` |
| [`crates/agent-gateway/src/routes/search.rs`](../crates/agent-gateway/src/routes/search.rs) | 227 | 2 | Iterates `registry.all()` to build search vectors |
| [`crates/agent-gateway/src/routes/capabilities.rs`](../crates/agent-gateway/src/routes/capabilities.rs) | 33 | (counted under others) | Returns `/v1/capabilities` JSON list — no rename to filename, only internal type updates |
| [`crates/common/src/error.rs`](../crates/common/src/error.rs) | — | 1 | `ConusAiError::Capability(String)` variant |
| [`crates/common/src/path_safety.rs`](../crates/common/src/path_safety.rs) | — | 2 | Doc comments mention "capability" |
| [`crates/agent-gateway/assets/js/app.js`](../crates/agent-gateway/assets/js/app.js) | — | 1 | One JSON field name; UI tool-card naming **already** uses `Tool` (`appendToolCard`/`finalizeToolCard`) |

**Workspace total:** 21 files, **105 `Capability*` occurrences**.

### Existing `tools/` module — naming collision

`crates/agent-core/src/tools/` already exists and contains:

```
tools/
├── mod.rs
├── fs_tools.rs           — read_file / write_file (tenant-scoped via safe_join)
├── cargo_tool.rs         — run_cargo (allowlisted subcommands)
└── native_capability.rs  — builds a CapabilityCard for the built-in fs+cargo tools
```

The v0.2 plan said "rename `capabilities/` → `tools/`" — **this would collide**. We resolve the collision in Phase 1 by moving the existing fs/cargo tools into a `builtin/` submodule, then renaming `capabilities/` → `tools/`. See Phase 1 §1.1.

### Existing trait surface

- `AgentCapability` trait exists in [`provider.rs`](../crates/agent-core/src/capabilities/provider.rs) but is **not implemented** by any of the dispatch targets (`InvoicePipeline`, `ContractPipeline`, `McpAdapter`, `WasmCapabilityLoader`). The match in `tool_executor.rs` bypasses it. Phase 2 deletes the unused trait and replaces it with `ToolProvider`.
- No `ExtractionPipeline` trait exists. `InvoicePipeline` and `ContractPipeline` are independent structs with different output types (`InvoiceData`, `ContractData`).

---

## Phase 0 — Preparation (5 min)

1. Branch from `main`:
   ```bash
   git switch -c refactor/tool-provider-alignment
   ```
2. Baseline test pass:
   ```bash
   cargo check --workspace
   cargo test --workspace 2>&1 | grep "test result"
   # expected: 30 passed (8 agent-core + 22 common; rest are 0-test crates)
   cargo clippy --workspace --all-targets -- -D warnings
   ```
3. Sanity-run the gateway against the live infra (Qdrant + MinIO already running locally per the user's session):
   ```bash
   curl -s http://localhost:8080/health
   curl -s http://localhost:8080/v1/capabilities | jq '.[].name'
   ```

**Rollback safety:** Each phase below ends with a green `cargo check --workspace`. If a phase breaks anything that earlier phases didn't break, revert that phase's commit and re-plan.

---

## Phase 1 — Mechanical rename `Capability*` → `Tool*` (45 min)

**Pure mechanical change. No behavioural change. No file deletions. After this phase the match in `tool_executor.rs` is unchanged in shape — only its types are renamed.**

### 1.0 Resolve the `tools/` collision

Move the existing built-in tools into a submodule so the new `tools/` (formerly `capabilities/`) can take its place:

```
crates/agent-core/src/tools/
├── mod.rs                          ← becomes parent of both groups
├── builtin/
│   ├── mod.rs                      ← `pub mod fs; pub mod cargo; pub mod card;`
│   ├── fs.rs                       ← was tools/fs_tools.rs
│   ├── cargo.rs                    ← was tools/cargo_tool.rs
│   └── card.rs                     ← was tools/native_capability.rs (renamed builtin_tool_card)
└── (then add the renamed capabilities/* files below)
```

Update imports:
- `crates/agent-core/src/capabilities/tool_executor.rs`: `use crate::tools::fs_tools` → `use crate::tools::builtin::fs`. Same for `cargo_tool` → `builtin::cargo`.
- `crates/agent-core/src/lib.rs`: `pub use tools::native_capability_card` → `pub use tools::builtin::card::builtin_tool_card`.
- `crates/agent-gateway/src/state.rs`: same.

### 1.1 Move + rename source files

```
crates/agent-core/src/capabilities/  →  crates/agent-core/src/tools/
```

Then rename within (use `git mv` so blame is preserved):

| Old path | New path |
|---|---|
| `capabilities/mod.rs` | `tools/mod.rs` (merge with existing `tools/mod.rs`) |
| `capabilities/provider.rs` | **deleted** (unused trait — see Phase 2) |
| `capabilities/manifest.rs` | `tools/manifest.rs` |
| `capabilities/card.rs` | `tools/card.rs` |
| `capabilities/registry.rs` | `tools/registry.rs` |
| `capabilities/discovery.rs` | `tools/discovery.rs` |
| `capabilities/embedding.rs` | `tools/embedding.rs` |
| `capabilities/mcp_adapter.rs` | `tools/mcp_adapter.rs` |
| `capabilities/tool_executor.rs` | `tools/executor.rs` (drop the `tool_` prefix — module is already `tools`) |
| `capabilities/wasm_loader.rs` | `tools/wasm_loader.rs` |

### 1.2 Symbol renames

| Old | New | Reason |
|---|---|---|
| `CapabilityManifest` | `ToolManifest` | Rig + MCP convention |
| `CapabilityKind` | `ToolKind` | (variants stay: `Mcp`, `Wasm`, `Pipeline`, `Docker`, `Native`) |
| `CapabilityCard` | `ToolCard` | matches UI's existing `appendToolCard` |
| `CapabilityRegistry` | `ToolRegistry` | |
| `CapabilityDiscovery` | `ToolDiscovery` | |
| `CapabilityExecutor` | `ToolExecutor` | |
| `WasmCapabilityLoader` | `WasmToolLoader` | |
| `native_capability_card()` | `builtin_tool_card()` | |
| `ConusAiError::Capability` (variant) | `ConusAiError::Tool` | one variant in `crates/common/src/error.rs` |
| `AgentCapability` (trait) | **deleted** | unused; replaced in Phase 2 |
| Module path `crate::capabilities::*` | `crate::tools::*` | |

Mechanical tool: run from repo root, then read each diff before committing (some matches will be in doc comments and the route name `/v1/capabilities` which **must not** change):

```bash
# Source code only — do NOT touch capability.yaml, /v1/capabilities path, capabilities/ folder under repo root
fd -e rs -E 'capabilities/*' . crates/ evals/ -x sd -- 'CapabilityManifest' 'ToolManifest' {}
fd -e rs -E 'capabilities/*' . crates/ evals/ -x sd -- 'CapabilityKind'     'ToolKind'     {}
fd -e rs -E 'capabilities/*' . crates/ evals/ -x sd -- 'CapabilityCard'     'ToolCard'     {}
fd -e rs -E 'capabilities/*' . crates/ evals/ -x sd -- 'CapabilityRegistry' 'ToolRegistry' {}
fd -e rs -E 'capabilities/*' . crates/ evals/ -x sd -- 'CapabilityDiscovery' 'ToolDiscovery' {}
fd -e rs -E 'capabilities/*' . crates/ evals/ -x sd -- 'CapabilityExecutor' 'ToolExecutor' {}
fd -e rs -E 'capabilities/*' . crates/ evals/ -x sd -- 'WasmCapabilityLoader' 'WasmToolLoader' {}
fd -e rs -E 'capabilities/*' . crates/ evals/ -x sd -- 'native_capability_card' 'builtin_tool_card' {}
fd -e rs -E 'capabilities/*' . crates/ evals/ -x sd -- 'crate::capabilities::' 'crate::tools::' {}
fd -e rs -E 'capabilities/*' . crates/ evals/ -x sd -- 'agent_core::capabilities::' 'agent_core::tools::' {}

# Hand-edit one occurrence each:
# - crates/common/src/error.rs            ConusAiError::Capability → ConusAiError::Tool
# - crates/agent-gateway/src/routes/mod.rs   route path "/v1/capabilities" stays
```

### 1.3 Things that DO NOT change

- Filenames `capabilities/*/capability.yaml` (these are external manifests).
- The `capabilities/` directory at the repo root (capability source layout — separate from the Rust module path).
- The `routes::capabilities` module name and the `/v1/capabilities*` HTTP path strings.
- `CONUSAI_CAPABILITIES_DIR` env var name (would break running deployments).

### 1.4 Verify

```bash
cargo check --workspace             # green
cargo test --workspace              # 30 passed
cargo clippy --workspace -- -D warnings
grep -rn 'Capability' crates/ evals/   # only doc comments + route path strings + env var name remain
```

**Commit:** `refactor: rename Capability* to Tool* (mechanical)`

---

## Phase 2 — Introduce `ToolProvider` trait + provider-based registry (90 min)

**Behavioural goal: zero diff. The agent loop, MCP dispatcher, search, and `/v1/capabilities` listing all behave identically.**

### 2.1 Define the trait

`crates/agent-core/src/tools/provider.rs`:

```rust
use crate::context::tenant::TenantContext;
use crate::tools::manifest::ToolDef;
use async_trait::async_trait;
use serde_json::Value;

/// Anything that can execute one or more named tools given a JSON input.
///
/// Implementors hold their own state (HTTP clients, WASM engines, model handles).
/// The registry keeps `Arc<dyn ToolProvider>` so providers can be cheap to clone
/// and shared across concurrent agent turns.
#[async_trait]
pub trait ToolProvider: Send + Sync + 'static {
    /// The manifest is the contract: name, kind, tool list, embedding text, etc.
    fn manifest(&self) -> &crate::tools::manifest::ToolManifest;

    /// Execute one tool and return its JSON output.
    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value>;

    /// Anthropic-format tool definitions; default implementation derives from the manifest.
    fn tool_definitions(&self) -> Vec<Value> {
        crate::tools::executor::tool_definitions_from_manifest(self.manifest())
    }
}
```

### 2.2 Implement `ToolProvider` for the four executor backends

One file per provider, each <100 lines. All keep their existing struct + methods so other call sites (e.g. `InvoicePipeline::extract_from_image_path` used by `ui/handlers/invoice.rs`) are untouched.

| New file | Wraps | Manifest source |
|---|---|---|
| `tools/providers/builtin.rs` | `crate::tools::builtin::{fs, cargo}` | Hard-coded manifest from `builtin_tool_card()` |
| `tools/providers/mcp.rs` | `McpAdapter` | The `ToolCard` loaded from `capability.yaml` (kind: mcp) |
| `tools/providers/wasm.rs` | `WasmToolLoader` | The `ToolCard` (kind: wasm) |
| `tools/providers/pipeline.rs` | `InvoicePipeline`, `ContractPipeline` | The `ToolCard` (kind: pipeline). One impl per pipeline; the registry decides which to instantiate from the manifest's `name` (`invoice-processing`, `contract-processing`, `ocr-service`). |

**Important:** keep the `(name, tool_name)` decisions inside each provider. For example, `InvoicePipelineProvider::invoke` does its own `match tool_name { "extract_invoice" => ..., "validate_invoice" => ... }`. This is now a per-provider concern, not a registry concern.

### 2.3 Refit the registry

`crates/agent-core/src/tools/registry.rs`:

```rust
pub struct ToolRegistry {
    cards:     HashMap<String, ToolCard>,                // name → metadata (kept for /v1/capabilities listing)
    providers: HashMap<String, Arc<dyn ToolProvider>>,   // name → executor
}

impl ToolRegistry {
    pub fn register(&mut self, provider: Arc<dyn ToolProvider>) {
        let name = provider.manifest().name.clone();
        self.cards.insert(name.clone(), ToolCard::from_manifest(provider.manifest().clone()));
        self.providers.insert(name, provider);
    }

    pub fn get_provider(&self, name: &str) -> Option<&Arc<dyn ToolProvider>> { ... }
    pub fn all(&self) -> impl Iterator<Item = &ToolCard> { self.cards.values() }
    // existing search_by_tag / len / is_empty unchanged
}
```

### 2.4 Provider factory used by `ToolDiscovery`

`crates/agent-core/src/tools/discovery.rs::discover()` already walks `CONUSAI_CAPABILITIES_DIR` and produces `ToolCard`s. Now it also instantiates the right provider:

```rust
fn provider_for(card: ToolCard) -> anyhow::Result<Arc<dyn ToolProvider>> {
    Ok(match card.manifest.kind {
        ToolKind::Mcp     => Arc::new(McpProvider::new(card)?),
        ToolKind::Wasm    => Arc::new(WasmProvider::new(card)?),
        ToolKind::Pipeline => match card.manifest.name.as_str() {
            "invoice-processing" => Arc::new(InvoiceProvider::new(card)),
            "contract-processing" => Arc::new(ContractProvider::new(card)),
            "ocr-service"        => Arc::new(OcrProvider::new(card)),
            other => anyhow::bail!("Unknown pipeline capability: {other}"),
        },
        ToolKind::Docker => anyhow::bail!("Docker kind reserved"),
        ToolKind::Native => Arc::new(BuiltinProvider::new()),
    })
}
```

**Adding a new pipeline-kind capability becomes "one new provider file + one match arm here."** That's the open-closed win.

### 2.5 Replace `ToolExecutor` call sites with provider lookup

The big match in `executor.rs` becomes:

```rust
pub async fn invoke(
    registry: &ToolRegistry,
    cap_name: &str,
    tool_name: &str,
    input: &Value,
    tenant: Option<&TenantContext>,
) -> anyhow::Result<Value> {
    let provider = registry
        .get_provider(cap_name)
        .ok_or_else(|| anyhow::anyhow!("No provider registered for '{cap_name}'"))?;
    provider.invoke(tool_name, input, tenant).await
}
```

Every existing call site that did:
```rust
ToolExecutor::invoke(card, tool_name, input, tenant).await
```
becomes:
```rust
ToolExecutor::invoke(&registry, &card.manifest.name, tool_name, input, tenant).await
```

Sites to update:
- [`crates/agent-gateway/src/routes/agent.rs`](../crates/agent-gateway/src/routes/agent.rs) line ~787 (`resolve_and_invoke`).
- [`crates/agent-gateway/src/routes/mcp.rs`](../crates/agent-gateway/src/routes/mcp.rs) `tools/call` handler.

The `cards: Vec<ToolCard>` snapshot used by the agent loop stays for tool-definition listing; only the invocation path changes.

### 2.6 Delete dead code

- Remove `crates/agent-core/src/tools/provider.rs`'s old `AgentCapability` trait (we already deleted it in Phase 1; double-check no stragglers).
- Delete `tool_definitions(card)` standalone function in favour of `provider.tool_definitions()` on the trait. Keep a shared `tool_definitions_from_manifest(manifest)` helper used by the default trait impl.

### 2.7 Verify

```bash
cargo check --workspace
cargo test --workspace                       # 30 passed
cargo clippy --workspace -- -D warnings

# Manual smoke through the live gateway:
curl -s -X POST http://localhost:8080/v1/agent/completions \
     -H 'Content-Type: application/json' -H 'X-Tenant-ID: dev' \
     -d '{"model":"claude-opus-4-7","messages":[{"role":"user","content":"run cargo check on this repo"}],"max_tokens":256}' \
     | jq '.choices[0].message.content'

# MCP path:
curl -s -X POST http://localhost:8080/mcp -H 'Content-Type: application/json' \
     -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | jq '.result | length'

# UI path:
# Open http://localhost:8080/?ws=<existing-conversation-ulid> and send a message
# that triggers wasm-ping (e.g. "run a ping test"). Confirm the toolCard renders
# and the streamed result shows ✅ Ping successful.
```

**Commit:** `refactor: replace tool_executor match with ToolProvider trait + provider-based registry`

---

## Phase 3 — `ExtractionPipeline<Output>` trait (45 min)

**Optional.** Skip if Phase 2 satisfies the open-closed concern. Recommended only if a 3rd extraction pipeline is on the roadmap.

The current pipelines (`InvoicePipeline`, `ContractPipeline`) and the OCR variant share a structural pattern — base64-encode bytes, send to Claude vision with a strict-JSON prompt, deserialize into a typed struct. Extracting the shared shape into a trait makes adding a 4th pipeline a one-file change.

```rust
// crates/agent-core/src/pipelines/extraction.rs (new)
#[async_trait]
pub trait ExtractionPipeline: Send + Sync {
    type Output: serde::de::DeserializeOwned + serde::Serialize + schemars::JsonSchema + Send;

    fn model(&self) -> &str;
    fn system_prompt(&self) -> &str;       // strict JSON schema directive
    fn schema(&self) -> serde_json::Value;  // JSON Schema for validation

    async fn extract_from_bytes(
        &self,
        bytes: Vec<u8>,
        mime: &str,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Self::Output>;
}
```

`InvoicePipeline` and `ContractPipeline` become trait impls; `extract_from_image_path` and `extract_from_document_path` become extension-trait helpers (one per pipeline if signatures genuinely diverge).

**This phase changes signatures used by:**
- `crates/agent-gateway/src/ui/handlers/invoice.rs`
- `crates/agent-core/src/tools/executor.rs` (now via providers)
- `evals/src/runners/invoice.rs`

Verify the same evals (`evals/datasets/invoice.jsonl`) still pass.

---

## Phase 4 — Polish (30 min, optional)

1. **Move `crates/invoice-demo` → `examples/invoice-cli`** to follow standard Cargo conventions. Update root `Cargo.toml`, `start.sh`, and `verify.md` references.
2. **Extract Qdrant boilerplate** — `crates/common/src/qdrant.rs` with a small `QdrantCollectionManager` (collection-create, payload-index-create, retry-on-409). Used by `qdrant_store.rs`, `qdrant_workspace_store.rs`, `qdrant_audit.rs` and the search route.
3. **In-memory test stores** — `InMemoryWorkspaceStore`, `InMemoryThreadStore`, `InMemoryAuditStore` behind `#[cfg(test)]` in `common::memory`. Lets `agent-gateway` integration tests run without docker. Wire `AppState::from_env_with_test_overrides()` to use them when `CONUSAI_TEST_MODE=1`.

---

## Phase 5 — Verification & merge (15 min)

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace                  # 30+ passed
cargo test --package evals              # if datasets present
./scripts/docker-verify.sh              # full e2e

# Doc updates
# - docs/arch.md §3.2: rename "Capabilities subsystem" → "Tools subsystem"; update file table
# - docs/arch.md §11: add a "ToolProvider trait" bullet in Design Patterns
# - docs/verify.md: add Phase 14 — Tool Provider regression checklist (smoke each ToolKind)
# - README.md: update terminology in any intro paragraphs

# Commit message
git commit -m 'refactor: align to ToolProvider + Rig.rs naming (no behaviour change)'
```

**Done when:**

- `grep -rn 'Capability' crates/ evals/` returns only:
  - Doc comments referring to capability YAML files
  - The route path string `"/v1/capabilities"` and `"/v1/capabilities/search"`
  - The env var `CONUSAI_CAPABILITIES_DIR`
  - Strings like `"capabilities/"` referring to the on-disk directory
- All 30 existing tests pass.
- `docker-verify.sh` exits 0.
- The UI works end-to-end: workspace tree loads, conversation opens, a chat message produces a streamed response, tool cards render for `wasm-ping`, search finds chat content.

---

## Risk register

| Risk | Likelihood | Mitigation |
|---|---|---|
| Phase 1 mass-rename touches a string inside a hot path (e.g. `"capabilities"` route literal) | Low | The `sd` commands are restricted to symbol patterns (`CapabilityX`); route paths and env vars use lowercase strings unchanged. Read every diff. |
| Phase 2 provider factory breaks the existing pipeline naming convention | Medium | Keep the existing `(name, tool_name)` keys inside each provider's `invoke`. The factory only routes by `kind` + `name` — the inner tool dispatch does not change. |
| Workspace + audit subsystems use `crate::capabilities::*` indirectly via `agent_core::*` re-exports | Low | `lib.rs` re-exports use the new names after Phase 1. Workspace store does not import capability types directly (verified via grep). |
| The UI's `appendToolCard` event flow depends on the `tool_call_start` SSE format from `agent.rs` | None | That format is JSON over SSE, not Rust types. Phase 1-2 don't touch the JSON wire format. |
| Tests skip integration coverage — refactor passes `cargo test` but breaks live behaviour | Medium | Phase 2 verify step explicitly hits `/v1/agent/completions`, `/mcp`, and the browser UI. `docker-verify.sh` re-runs full e2e in Phase 5. |
| Pipeline provider needs a per-tenant config but currently relies on `pipeline.with_tenant(t.clone())` per call | Medium | Provider keeps the same per-call pattern: `InvoiceProvider::invoke` constructs a fresh pipeline per call (cheap — just a struct + model id). Defer pooling to a future phase. |

---

## Out of scope (intentional)

- New tool kinds (`Docker`, `Rag`, etc.) — the refactor enables them but does not add them.
- Schema-versioned manifests (no `manifest_version` field bumps).
- Anthropic SDK upgrades.
- `crates/common/src/error.rs` further changes beyond renaming `Capability(String)` → `Tool(String)`.
- Audit log instrumentation gaps (separate task — see arch.md §3.2).
- Workspace ACL changes (separate ADR — `docs/adr/005-workspace-access-control.md`).

---

## Effort summary

| Phase | Wall time | Risk | Reversibility |
|---|---:|---|---|
| 0 — Prep | 5 min | None | Trivial |
| 1 — Mechanical rename | 45 min | Low | Single revert commit |
| 2 — `ToolProvider` trait + registry | 90 min | Medium | Single revert commit |
| 3 — `ExtractionPipeline` (optional) | 45 min | Medium | Single revert commit |
| 4 — Polish (optional) | 30 min | Low | Per-item revertible |
| 5 — Verify + docs + merge | 15 min | None | — |
| **Total (required)** | **~2.5 h** | | |
| **Total (with optional)** | **~4 h** | | |

Original v0.2 estimate was 65 min; the v0.3 estimate is higher because the v0.2 estimate undercounted the 21 affected files, the `tools/` collision, the second pipeline (`ContractPipeline` was added after v0.2), and the 8-arm dispatch (v0.2 assumed 3).
