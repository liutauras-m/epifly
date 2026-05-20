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

If your capability needs `Arc<dyn WorkspaceStore>`, add `op = "..."` to the `[config]` section and handle it in `NativeStorageFactory::create()` in `agent-core/src/capabilities/providers/native_storage.rs`.

### Job-backed capabilities (async, long-running)

If your capability should enqueue a background job, register it programmatically in `agent-gateway/src/state.rs` using `registry.register_provider()` after the job executor is built. See `transcribe-video` as the reference implementation.

### MCP server capabilities

Deploy an MCP server, then `POST /admin/capabilities/register` with the endpoint URL. The gateway supports hot-registration without restart.
