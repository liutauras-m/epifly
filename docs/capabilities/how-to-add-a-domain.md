# How to Add a New Domain Capability

Four steps. No Rust changes required for most cases.

---

## Step 1 — Pick a namespace and kind

Choose where in the [taxonomy](taxonomy.md) your capability fits:

```
storage.* | compute.* | sense.* | extract.* | convert.* | compose.* | deliver.* | plan.*
```

Choose a `kind`:

| Kind | Use when |
|---|---|
| `chain` | The capability is a prompt sent to an LLM |
| `mcp` | The capability calls an MCP-protocol tool server |
| `wasm` | The capability runs a WASM module |
| `native` | The capability needs Rust code with access to workspace storage |

---

## Step 2 — Create the TOML manifest

Create `apps/backend/capabilities/<your-name>/capability.toml`:

```toml
schema_version = "2.0"
name        = "extract-purchase-order"
version     = "0.1.0"
namespace   = "extract.fields.purchase_order"
category    = "extract"
kind        = "chain"
description = "Extract structured fields from a purchase order document."
tags        = ["procurement", "extraction", "purchase-order"]
accepts     = ["application/pdf", "image/*"]
emits       = ["application/json"]
idempotent  = true
requires    = []
search_keywords = ["purchase order", "PO", "procurement", "extract"]

[[tools]]
name = "extract_po"
description = "Extract fields from a purchase order. Returns JSON with vendor, amount, line_items."

[tools.input_schema]
type = "object"
required = ["image_path"]

[tools.input_schema.properties.image_path]
type = "string"
description = "Path to the purchase order image or PDF"

[chain]
model = "claude-opus-4-7"
system_prompt = """
You are a purchase order extraction specialist. Extract all relevant fields
from the provided document. Return a JSON object with these fields:
- vendor_name, po_number, po_date, total_amount, currency, line_items
"""
prompt_template = "Extract purchase order data from: {{input.image_path}}"
max_tokens = 2048
```

---

## Step 3 — Validate the manifest

```bash
cargo xtask capabilities lint
```

Fix any reported issues (missing required fields, invalid namespace root, etc.).

---

## Step 4 — Verify discovery

Restart the gateway (or wait for hot-reload) and check:

```bash
curl http://localhost:8080/v1/capabilities | jq '.[] | select(.name == "extract-purchase-order")'
```

The capability will appear in the agent's tool catalog and be searchable by the `SemanticCapabilityRouter` immediately.

---

## Special cases

### Native capabilities (require workspace storage)

**Single-tool (legacy pattern — do not use for new work):** add `op = "..."` to the
`[config]` section and handle it in `NativeStorageFactory::create()` in
`agent-core/src/capabilities/providers/native_storage.rs`.

**Multi-tool native (preferred pattern):** implement a purpose-built provider struct
(e.g. `StorageWorkspaceProvider`) in `native_storage.rs` whose `invoke()` dispatches
on `tool_name`. No `[config] op` is needed — just declare `[[tools]]` blocks in the
TOML and add a `manifest.name` match in `NativeStorageFactory::create()`:

```rust
match card.manifest.name.as_str() {
    "my-domain" => return Ok(Arc::new(MyDomainProvider::new(card.manifest, /* deps */))),
    _ => {}
}
```

`ToolManifest` schema is **unchanged** — only standard `[[tools]]` blocks are used;
no `[[config.tools]]` extension exists or is needed. See `StorageWorkspaceProvider`
and `StorageFsProvider` in `native_storage.rs` for reference implementations.

### Job-backed capabilities (async, long-running)

If your capability should enqueue a background job, register it programmatically in `agent-gateway/src/state.rs` using `registry.register_provider()` after the job executor is built. See `transcribe-video` as the reference implementation.

### MCP server capabilities

Deploy an MCP server, then `POST /admin/capabilities/register` with the endpoint URL. The gateway supports hot-registration without restart.

### Checklist (post-PR 3/4)

When adding a new capability, also do the following so it interoperates with routing, live state, and the parity invariants:

- **Populate `[[tools]].search_keywords`** for any tool with strong lexical triggers (delete / upload / scaffold / etc.). The router reads keywords from the manifest — no in-code table. Drift is caught by the regression suite at `apps/backend/crates/agent-gateway/tests/routing_quality.rs` (PR 4.2).
- **Publish to `InvalidationBus`** from any state-mutating tool so live consumers refresh automatically. Use `state.invalidation_bus.send(InvalidationEvent::new(<resource>, &tenant_id).with_keys(<changed paths>))`. Existing producers: `ArtifactBridge::process_if_artifacts` → `"workspace"`, `stream_agent` end-of-turn → `"threads"` (PR 3.A.6). See [`docs/arch.md`](../arch.md) §18.
- **Any feature `.svelte` ships in `packages/ui`**, never in `apps/<app>`. The cross-app parity invariant (`docs/plan.md` §0.5) is enforced by `scripts/check-cross-app-imports.mjs` (CI fails on violation).
- **Add 1–2 routing fixtures** to `apps/backend/crates/agent-gateway/tests/fixtures/routing_prompts.toml` whenever you add a capability with distinct user-intent prompts. Append the case `id` to `tests/fixtures/routing_baseline.txt` after confirming it passes locally.
