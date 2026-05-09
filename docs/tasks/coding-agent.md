**Yes — implementing a full-featured Coding Agent usable directly from chat is straightforward, idiomatic, and 100 % aligned with v0.3 + Rig.rs v0.36+ standards.**

We treat the Coding Agent exactly like every other specialized agent in the platform: a **pre-configured `Agent`** assembled via `AgentBuilder` that bundles a **single `CodingCapability`** (the canonical `CapabilityProvider` for this behavior).  
This gives us:
- SRP (one capability = one responsibility)
- Discoverability via `/v1/capabilities` + Qdrant semantic search
- Immediate availability in `/v1/chat/completions` and `/v1/agent/completions`
- Full reuse of workspace, rig-qdrant, WASI sandbox, and observability layers

### Recommended Architecture (v0.3 canonical)

```bash
crates/agent-core/
├── src/
│   ├── capabilities/
│   │   └── coding/
│   │       ├── mod.rs
│   │       ├── capability.rs          # CodingCapability impl
│   │       ├── tools/                 # ReadWorkspaceFileTool, etc.
│   │       └── prompts/
│   │           └── coding_agent.system.md
│   ├── agents/
│   │   └── coding.rs                  # convenience builder
│   └── lib.rs                         # re-exports
```

(No new top-level crate needed yet — we keep it inside `agent-core` for zero-overhead MVP. We can extract to `crates/coding-capability` later if the community wants to publish it.)

### 1. `CodingCapability` (core abstraction)

```rust
// crates/agent-core/src/capabilities/coding/capability.rs
use crate::capabilities::{CapabilityCard, CapabilityProvider};
use async_trait::async_trait;
use rig_core::Tool; // Rig v0.36+ Tool trait
use std::sync::Arc;

#[derive(Clone)]
pub struct CodingCapability {
    workspace: Arc<WorkspaceService>,           // injected from agent-gateway
    code_index: Arc<dyn rig_qdrant::VectorStoreIndex>, // RAG over workspace files
    wasi_env: Arc<WasmtimeWasiEnvironment>,     // safe sandbox
}

#[async_trait]
impl CapabilityProvider for CodingCapability {
    fn card(&self) -> CapabilityCard {
        CapabilityCard {
            name: "coding".into(),
            description: "Full-cycle software engineering agent (plan → code → edit → test → debug)".into(),
            version: "0.3.0".into(),
            category: "development".into(),
            requires_workspace: true,
            // … other metadata
        }
    }

    fn tools(&self) -> Vec<Box<dyn Tool>> {
        vec![
            Box::new(tools::ReadWorkspaceFileTool::new(self.workspace.clone())),
            Box::new(tools::WriteWorkspaceFileTool::new(self.workspace.clone())),
            Box::new(tools::ListWorkspaceTreeTool::new(self.workspace.clone())),
            Box::new(tools::SemanticCodeSearchTool::new(self.code_index.clone())), // rig-qdrant powered
            Box::new(tools::SandboxExecuteTool::new(self.wasi_env.clone())),      // cargo check, tests, python, etc.
            // future: git ops, PR creation, etc.
        ]
    }

    // Optional advanced hooks (Rig v0.30+ style)
    async fn on_tool_call_hook(&self, tool_name: &str, args: &str) -> rig_core::ToolCallHookAction {
        // permission / rate-limit logic if needed
        rig_core::ToolCallHookAction::Continue
    }
}
```

### 2. Pre-configured `CodingAgent` via `AgentBuilder`

```rust
// crates/agent-core/src/agents/coding.rs
use crate::AgentBuilder;
use crate::capabilities::coding::CodingCapability;
use rig_core::providers::CompletionModel; // or your CompletionProvider trait

pub fn build_coding_agent(
    model: impl CompletionModel,
    capability: CodingCapability,
) -> Agent {
    AgentBuilder::new(model)
        .name("coding-agent")
        .preamble(include_str!("../capabilities/coding/prompts/coding_agent.system.md"))
        .capability(capability)          // our extension point on top of Rig tools
        .temperature(0.1)                // deterministic for code
        .max_turns(25)                   // Rig v0.30+ renamed API
        .build()
}
```

(The system prompt is a strong ReAct / Plan-Execute-Verify loop tailored for Rust + workspace context — we can iterate on it with evals later.)

### 3. Registration & Chat Integration (zero breaking changes)

- On startup / via `/admin/capabilities` the `CapabilityFactory` registers `"coding"` exactly like every other capability.
- `agent-gateway` already resolves capabilities by name.  
  Users can now call the Coding Agent in two natural ways:

```json
// POST /v1/chat/completions
{
  "model": "coding-agent",           // or any model + @coding mention
  "messages": [ ... ],
  "workspace_id": "ws_01JXYZ..."     // optional but recommended
}

// or the dedicated endpoint
// POST /v1/agent/completions
{ "agent": "coding-agent", ... }
```

The runtime looks up the pre-built `Agent` from the registry, injects the current workspace context into the tools, and streams the response exactly like any other agent.

### Security & Observability (already covered)

- All file I/O goes through the existing workspace service + audit log.
- Code execution is fully sandboxed via `wasmtime-wasi` (Preview 2).
- Every tool call is traced with OpenTelemetry + semantic GenAI conventions.


