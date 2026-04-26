**CONUSAI PLATFORM — AGGRESSIVE v0.3 IMPROVEMENTS IMPLEMENTATION PLAN**  
**Target:** Merge the best of both architectures into a **Rig-first, capability-extensible, self-hosted Ollama powerhouse** while keeping independent deployability.  
**Deadline mindset:** Ship production-grade in < 3 weeks. Every step has verification. No shortcuts. SRP enforced at every layer. Follow Rig 0.26+ patterns strictly.

---

### ✅ COMPLETED: Capability Description Quality Pass (2026-04-26)

**Problem:** `ocr-service` and `invoice-processing` had ambiguous descriptions that could lead the LLM to make redundant tool calls (e.g. running ocr-service before invoice-processing).

**Solution applied:** Rewrote `description` and per-tool `description` fields in both `capability.yaml` files to be explicitly directive:
- `invoice-processing/capability.yaml` — now explicitly states it handles the vision step internally; tells the LLM to use it for any invoice/bill/facture document; explicitly says "Do NOT chain ocr-service before this".
- `ocr-service/capability.yaml` — now explicitly scopes itself to generic/raw-text needs only; tells the LLM to prefer `invoice-processing__extract_invoice` for any structured document.

**Why this works:** `CapabilityManifest` descriptions are loaded verbatim into Anthropic tool definitions at startup via `tool_definitions()` in `tool_executor.rs`. Rich natural-language tool descriptions are the gold-standard mechanism for deterministic tool routing in Rig 0.9+ / Anthropic tool-calling — no code-level classifier needed.

**Verified:** Live test with `invoice.png` confirmed the agent correctly chose `invoice-processing__extract_invoice` directly (514ms download → 359ms extract → 998ms presigned URL) without redundant `ocr-service` call.

---

**Prerequisites (Run Once)**
```bash
cd /path/to/conusai-platform
cargo update
cargo install cargo-nextest cargo-llvm-cov
./start.sh full  # ensure everything is green before touching code
```

---

### PHASE 1: LIGHTWEIGHT CAPABILITY SYSTEM (2–3 days)

**Goal:** Zero-code extensibility for new services/tools without touching agent-core.

**1.1 Create Capability Manifest & Registry (SRP)**

Create:
```bash
mkdir -p crates/agent-core/src/capabilities
touch crates/agent-core/src/capabilities/{mod.rs,manifest.rs,registry.rs,provider.rs}
```

**manifest.rs** (exact content):
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub kind: CapabilityKind,
    pub tools: Vec<ToolDef>,
    pub base_url: Option<String>,  // for HTTP services
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CapabilityKind { HttpService, InProcess, Wasm }  // start with HttpService

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}
```

**registry.rs**:
```rust
use std::collections::HashMap;
use std::sync::Arc;

pub struct CapabilityRegistry {
    capabilities: HashMap<String, CapabilityManifest>,
}

impl CapabilityRegistry {
    pub fn new() -> Self { ... }
    pub fn register(&mut self, manifest: CapabilityManifest) { ... }
    pub fn get(&self, name: &str) -> Option<&CapabilityManifest> { ... }
    pub fn all_tools(&self) -> Vec<ToolDef> { ... }  // flatten for Rig
}
```

**provider.rs** — `AgentCapability` trait (Rig-compatible):
```rust
#[async_trait::async_trait]
pub trait AgentCapability: Send + Sync {
    fn name(&self) -> &str;
    async fn invoke(&self, tool: &str, input: serde_json::Value) -> Result<serde_json::Value>;
}
```

**1.2 Update agent-core/Cargo.toml**
```toml
[dependencies]
rig-core = "0.26"
serde_yaml = "0.9"
# ... existing
```

**1.3 Discovery from capabilities/ folder (like original design)**
Add `discovery.rs` that scans `capabilities/` for `capability.yaml` and registers HTTP services (OCR, VideoTS, future ones).

**Verification Phase 1**
```bash
cargo test --package agent-core capabilities
./scripts/test_capability_registry.sh  # must print 3 capabilities + tools
cargo clippy --package agent-core -- -D warnings
```

---

### PHASE 2: DEEPER RIG 2026 INTEGRATION (2 days)

**Goal:** Replace manual Ollama client with full Rig `AgentBuilder` + dynamic tools.

**2.1 Refactor agent-core/agent.rs**

Replace `run_general_agent` with:
```rust
use rig::agent::AgentBuilder;
use rig::providers::ollama;

pub async fn build_conusai_agent(
    config: &AppConfig,
    registry: &CapabilityRegistry,
) -> Result<Agent<ollama::CompletionModel>> {
    let client = ollama::Client::new(&config.ollama_api_base_url);
    let model = client.model(&config.ollama_model);

    let mut builder = model
        .agent()
        .preamble("You are ConusAI, a general-purpose engineering agent...")
        .context(format!("Workspace root: {}", config.workspace_root));

    // Dynamically register ALL tools from registry + hardcoded coding tools
    for tool_def in registry.all_tools() {
        builder = builder.tool(ConusaiToolAdapter::new(tool_def));
    }

    // Add coding tools (ReadFile, WriteFile, RunCargo) as native Rig tools
    builder = builder
        .tool(ReadFileTool::new(config.workspace_root.clone()))
        .tool(WriteFileTool::new(config.workspace_root.clone()))
        .tool(RunCargoTool::new());

    Ok(builder.build())
}
```

**2.2 Create Tool Adapters (SRP)**
`crates/agent-core/src/tools/adapters.rs` — one adapter per capability kind. Implements Rig `Tool` trait.

**Verification Phase 2**
```bash
cargo test --package agent-core rig_agent
./scripts/test_agent_gateway_capability_chat.sh  # must now use new Rig path
# Check logs: "Registered 8 tools dynamically"
```

---

### PHASE 3: THREADS & MEMORY (3 days — Highest Priority)

**Goal:** Multi-turn conversations with persistent context (OpenAI Assistants parity).

**3.1 Add Thread Module**
```bash
mkdir -p crates/agent-core/src/context/threads
touch crates/agent-core/src/context/threads/{mod.rs,store.rs,thread.rs}
```

**thread.rs**:
```rust
#[derive(Clone, Serialize, Deserialize)]
pub struct Thread {
    pub id: Ulid,
    pub tenant_id: String,
    pub messages: Vec<Message>,
    pub summary: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
}
```

**store.rs** — `ThreadStore` trait + SQLite implementation (for self-hosted simplicity; later swap to Qdrant):
```rust
#[async_trait]
pub trait ThreadStore: Send + Sync {
    async fn create(&self, tenant: &str) -> Result<Thread>;
    async fn get(&self, thread_id: &str) -> Result<Option<Thread>>;
    async fn append(&self, thread_id: &str, msg: Message) -> Result<()>;
    async fn summarize(&self, thread_id: &str) -> Result<String>;  // LLM-as-Judge
}
```

Use `sqlx` + `rusqlite` for local file-based persistence under `workspaces/{tenant}/threads.db`.

**3.2 Wire into AgentRuntime**
Add `with_thread(thread_id)` to builder and inject history into Rig preamble/context.

**3.3 API Update (agent-gateway)**
Add routes:
- `POST /v1/threads`
- `GET /v1/threads/{id}`
- `POST /v1/agent/completions` (always uses full tool loop + threads)

**Verification Phase 3**
```bash
cargo test --package agent-core threads
./scripts/test_threads.sh  # create thread → 5 turns with OCR + code tools → verify summary generated
curl -X POST http://localhost:8080/v1/threads | jq '.id'
# Must survive gateway restart
```

---

### PHASE 4: STREAMING + TOOLS RECONCILIATION (1–2 days)

**Goal:** Never lose tool power in streaming.

**Solution:** New endpoint `/v1/agent/completions` (non-streaming JSON by default).  
For streaming: Use Rig streaming + emit tool calls as special SSE events, then continue.

Update `api.rs`:
```rust
if stream {
    // Rig streaming path with tool loop (new in 0.26+)
} else {
    // full agent path
}
```

**Verification**
Run both streaming and non-streaming capability tests — tools must fire in both.

---

### PHASE 5: OBSERVABILITY, EVALS & POLISH (2 days)

**5.1 Full OTEL**
Wire `tracing-opentelemetry` + Jaeger in all services. Add spans for every tool call + thread operation.

**5.2 Evals Crate (revive from original)**
```bash
mkdir -p evals
# Add runners for threads, ocr_quality, code_tool_correctness
cargo run --bin evals -- run --suite threads
```

**5.3 Config Modernization**
Replace raw env with `figment` in all crates.

**5.4 Docker Polish**
Add `cargo-chef` to Dockerfiles for 10x faster rebuilds.

**Verification Phase 5**
```bash
cargo llvm-cov --workspace --html  # >92% coverage required
./scripts/test_docker_local_mac.sh
./scripts/test_evals.sh
# Jaeger UI must show full trace with thread_id + tool spans
```

---

### GLOBAL VERIFICATION SUITE (Run After Every Phase)

Create `scripts/verify_v0_3.sh`:
```bash
#!/bin/bash
set -euo pipefail

echo "=== Phase Verification ==="
cargo fmt -- --check
cargo clippy --workspace -- -D warnings
cargo nextest run --workspace
./start.sh full
./scripts/test_all_agents_health.sh
./scripts/test_agent_gateway_capability_chat.sh
./scripts/test_threads.sh
cargo run --bin evals -- run --suite all
echo "✅ v0.3 VERIFIED — All systems green"
```

**Aggressive Rules**
- Every new file must have 100% doc comments.
- All public APIs must have examples.
- No `unwrap()` in production paths — use `?` + proper error mapping.
- Performance gate: agent response < 8s on 26B model with 3 tools.

**Rollback Plan**
Every phase has a git tag: `git tag v0.3-phase1-ready`

**Final Command After Completion**
```bash
git commit -m "feat: v0.3 — Rig-first + Capabilities + Threads (production ready)"
./start.sh prod-amd-gpu
```

