# Capability Authoring Guide

A **capability** is any tool the AI agent can invoke at runtime. There are two ways to add one:

| Approach | When to use |
|---|---|
| [TOML file](#option-a-toml-capability-file) | Logic already lives inside the gateway (chain prompts, WASM, native Rust) |
| [Self-registering MCP service](#option-b-self-registering-mcp-service) | Logic runs in a separate process / language / container |

This guide focuses on **Option B** — a standalone HTTP microservice that registers itself at startup.  
No gateway restart is required. No config files need editing. No gateway code needs changing.

---

## How it works end-to-end

```
┌────────────────────────────────────────────────────────┐
│  Your service (any language)                           │
│                                                        │
│  1. On startup → POST /admin/capabilities/register     │
│     to the gateway with your tool manifest             │
│                                                        │
│  2. Expose POST /mcp  (MCP JSON-RPC 2.0)               │
│     handles tools/list and tools/call                  │
└────────────────────────────────────────────────────────┘
         │ register                      ▲ tools/call
         ▼                               │
┌─────────────────────────────────────────────────────────┐
│  ConusAI Gateway                                        │
│                                                        │
│  • Adds the tools to the in-memory registry             │
│  • Embeds the descriptions (fastembed) into Qdrant      │
│  • Calls only endpoints on CONUSAI_MCP_ALLOWED_HOSTS    │
│                                                        │
│  At query time:                                        │
│    user message → fastembed ANN → top-K tools          │
│    → Anthropic (tool call) → gateway → POST /mcp       │
│    → agent uses result in final response               │
└─────────────────────────────────────────────────────────┘
```

---

## Capability kinds (`ToolKind`)

Every capability TOML file and self-registration manifest must declare a `kind`. The full set of supported kinds is:

| Kind | When to use |
|---|---|
| `chain` | Data-driven LLM prompt — logic lives entirely in TOML (`[chain]` block), no Rust needed. Best choice for most new capabilities. |
| `wasm` | Compiled WebAssembly module bundled with `capability.toml` — for compute-heavy sandboxed logic. |
| `native` | Built-in in-process tools (e.g. filesystem helpers, cargo runner). **Not loaded from TOML** — registered at startup in code. |
| `mcp` | In-process MCP adapter that reads a local `capability.toml` and routes calls to a co-located MCP binary. |
| `docker` | Capability that spins up a Docker container per invocation. |
| `dynamic_prompt` | DB-backed, versioned prompt capability — store and version prompt templates in redb, hot-reload without restarting the gateway. |
| `remote_mcp` | External MCP service registered via JSON (no TOML on disk). The most common choice for self-registering microservices — see [Option B](#option-b-self-registering-mcp-service) below. |

---

## Option A: TOML capability file

Drop a `capability.toml` into `apps/backend/capabilities/<name>/` and restart the gateway.  
Use this for LLM chain tools, WASM tools, or native Rust tools.

```toml
name        = "my-tool"
version     = "1.0.0"
description = "Does something useful. Called when the user asks about X."
kind        = "chain"           # chain | wasm | native | mcp | docker | dynamic_prompt
tags        = ["tag1", "tag2"]

[[tools]]
name        = "my_function"
description = "Detailed description — this is what the semantic router reads."

[tools.input_schema]
type     = "object"
required = ["query"]

[tools.input_schema.properties.query]
type        = "string"
description = "The user query to process"

[chain]
model           = "claude-opus-4-7"
system_prompt   = "You are a helpful assistant."
prompt_template = "Answer this: {{input.query}}"
max_tokens      = 512
```

---

## Option B: Self-registering MCP service

### Step 1 — Implement the MCP JSON-RPC endpoint

Your service needs one HTTP route: `POST /mcp`

It must handle two JSON-RPC 2.0 methods:

#### `tools/list` — return your tool definitions

Request:
```json
{ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }
```

Response:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "tools": [
      {
        "name": "my_function",
        "description": "What this tool does",
        "inputSchema": {
          "type": "object",
          "properties": {
            "param1": { "type": "string", "description": "..." }
          },
          "required": ["param1"]
        }
      }
    ]
  }
}
```

#### `tools/call` — execute a tool

Request:
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "my_function",
    "arguments": { "param1": "value" }
  }
}
```

Response (success):
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "content": "The result as a human-readable string",
    "artifacts": [],
    "metadata": { "any": "extra data" }
  }
}
```

Response (tool not found):
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "error": { "code": -32601, "message": "Unknown tool: my_function" }
}
```

> **Important:** `params.name` in `tools/call` is the tool's function name (e.g. `"my_function"`),  
> **not** the service name. Match it exactly to what you declared in `tools/list`.

---

### Step 2 — Self-register on startup

On startup, POST this JSON to `{GATEWAY_URL}/admin/capabilities/register`:

```json
{
  "capability_id": "acme.billing.invoice-parser",
  "namespace":     "acme.billing",
  "version":       "1.0.0",
  "kind":          "remote_mcp",
  "endpoint":      "http://invoice-parser:8090/mcp",
  "tools": [
    {
      "name":        "parse_invoice",
      "description": "Extract line items, totals and vendor info from an invoice PDF or image URL.",
      "input_schema": {
        "type": "object",
        "required": ["url"],
        "properties": {
          "url": { "type": "string", "description": "Public URL of the invoice file" }
        }
      }
    }
  ],
  "tags":         ["invoice", "finance", "parsing"],
  "global":       true,
  "enabled":      true
}
```

#### Registration fields

| Field | Required | Description |
|---|---|---|
| `capability_id` | ✅ | Reverse-DNS id, `[a-z][a-z0-9._-]*`, max 128 chars. |
| `namespace` | ✅ | Dot-separated `[a-z][a-z0-9_]*` segments (max 6): `"acme.billing"`, `"media.time"`. Each tool becomes the capability `<namespace>.<tool name>`. |
| `version` | ✅ | Version string (max 64 chars). |
| `kind` | ✅ | Must be `"remote_mcp"`. |
| `endpoint` | ✅ | Full URL the gateway calls for `tools/list` and `tools/call`. Its host (or host:port) must be listed in the gateway's `CONUSAI_MCP_ALLOWED_HOSTS`. |
| `tools` | ✅ | 1 to 32 tool definitions (see below). |
| `tenant_scope` | one of | Tenant ids (Zitadel organization ids) that may see the tools. |
| `global` | one of | `true` exposes the tools to every tenant. Set exactly one of `tenant_scope` or `global`. |
| `tags` | — | Up to 16 tags of 1 to 64 characters. |
| `enabled` | — | Defaults to `true`. Set `false` to register but not expose yet. |

Unknown fields are refused. A capability name that is already registered is never replaced:
the gateway answers `409 Conflict`.

#### Tool definition fields

| Field | Required | Description |
|---|---|---|
| `name` | ✅ | Function name: `[a-z0-9_-]`, 1 to 64 characters, no dots. Must match what your `tools/call` handler expects. |
| `description` | ✅ | Up to 2000 characters. This is what the semantic router embeds and ranks. Write it from the user's perspective — describe *when to use this*, not just what it does. |
| `input_schema` | ✅ | JSON Schema object (`type: "object"` with `properties`). |

> **Tip:** Tool descriptions are the most important field for routing accuracy.  
> "Extract structured fields from an invoice document — use when the user uploads a bill, receipt, or purchase order"  
> is far better than "processes invoice".

---

### Step 3 — Add retry logic

The gateway may not be ready when your service starts. Implement retries:

```python
# Python example
async def register_with_retry(max_retries=10, delay=3.0):
    url = f"{os.environ['GATEWAY_URL']}/admin/capabilities/register"
    headers = {"Authorization": f"Bearer {os.environ['PLATFORM_ADMIN_TOKEN']}"}

    for attempt in range(1, max_retries + 1):
        try:
            async with httpx.AsyncClient(timeout=10) as client:
                resp = await client.post(url, json=MANIFEST, headers=headers)
                resp.raise_for_status()
                print(f"Registered: {resp.json()}")
                return
        except Exception as e:
            print(f"Registration attempt {attempt}/{max_retries} failed: {e}")
            if attempt < max_retries:
                await asyncio.sleep(delay)

    print("WARNING: all registration attempts failed — running unregistered")
```

---

### Step 4 — Environment variables

| Variable | Description |
|---|---|
| `GATEWAY_URL` | Base URL of the gateway, e.g. `http://gateway:8080` inside the compose project |
| `PLATFORM_ADMIN_TOKEN` | The gateway's `PLATFORM_ADMIN_TOKEN`, sent as `Authorization: Bearer`. Always required. |
| `SERVICE_URL` | Your own service's URL as the gateway reaches it (used in `endpoint` in the manifest). |

---

## Complete Python example

```python
"""
my-tool — self-registering MCP capability service.
"""
import asyncio, os
from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse
import httpx

app = FastAPI(title="my-tool")

# ── Tool definition ────────────────────────────────────────────────────────────

TOOL_DEF = {
    "name": "my_function",
    "description": "Does X. Call this when the user asks about Y.",
    "inputSchema": {
        "type": "object",
        "required": ["query"],
        "properties": {
            "query": {"type": "string", "description": "The input query"}
        },
    },
}

# ── MCP handler ────────────────────────────────────────────────────────────────

@app.post("/mcp")
async def mcp(request: Request):
    try:
        body = await request.json()
    except Exception:
        return JSONResponse(status_code=400, content={
            "jsonrpc": "2.0", "id": None,
            "error": {"code": -32700, "message": "Parse error"}
        })

    req_id = body.get("id")
    method = body.get("method", "")
    params = body.get("params") or {}

    if method == "tools/list":
        return {"jsonrpc": "2.0", "id": req_id, "result": {"tools": [TOOL_DEF]}}

    if method == "tools/call":
        name = params.get("name")
        args = params.get("arguments") or {}

        if name == "my_function":
            result = await run_my_function(args.get("query", ""))
            return {"jsonrpc": "2.0", "id": req_id, "result": result}

        return {"jsonrpc": "2.0", "id": req_id,
                "error": {"code": -32601, "message": f"Unknown tool: {name}"}}

    return {"jsonrpc": "2.0", "id": req_id,
            "error": {"code": -32601, "message": f"Method not found: {method}"}}


async def run_my_function(query: str) -> dict:
    # Your actual logic here
    return {
        "content": f"Result for: {query}",
        "artifacts": [],
        "metadata": {},
    }

# ── Self-registration ──────────────────────────────────────────────────────────

SERVICE_URL = os.getenv("SERVICE_URL", "http://my-tool:8090")

MANIFEST = {
    "capability_id": "acme.tools.my-tool",
    "namespace":     "acme.tools",
    "version":       "1.0.0",
    "kind":          "remote_mcp",
    "endpoint":      f"{SERVICE_URL}/mcp",
    "tools":         [{"name": "my_function",
                       "description": TOOL_DEF["description"],
                       "input_schema": TOOL_DEF["inputSchema"]}],
    "tags":          ["my-tag"],
    "global":        True,
    "enabled":       True,
}

async def register_with_retry(max_retries=10, delay=3.0):
    gateway = os.environ["GATEWAY_URL"]
    headers = {"Authorization": f"Bearer {os.environ['PLATFORM_ADMIN_TOKEN']}"}

    for attempt in range(1, max_retries + 1):
        try:
            async with httpx.AsyncClient(timeout=10) as client:
                r = await client.post(f"{gateway}/admin/capabilities/register",
                                      json=MANIFEST, headers=headers)
                r.raise_for_status()
                print(f"[my-tool] registered: {r.json()}")
                return
        except Exception as e:
            print(f"[my-tool] attempt {attempt}/{max_retries} failed: {e}")
            if attempt < max_retries:
                await asyncio.sleep(delay)

    print("[my-tool] WARNING: running unregistered")


@app.on_event("startup")
async def on_startup():
    asyncio.create_task(register_with_retry())
```

---

## Complete Node.js example

```typescript
import express from "express";
import axios from "axios";

const app = express();
app.use(express.json());

const TOOL_DEF = {
  name: "my_function",
  description: "Does X. Call this when the user asks about Y.",
  inputSchema: {
    type: "object",
    required: ["query"],
    properties: {
      query: { type: "string", description: "The input query" },
    },
  },
};

app.post("/mcp", async (req, res) => {
  const { id, method, params } = req.body;

  if (method === "tools/list") {
    return res.json({ jsonrpc: "2.0", id, result: { tools: [TOOL_DEF] } });
  }

  if (method === "tools/call") {
    const { name, arguments: args } = params ?? {};
    if (name === "my_function") {
      const result = await runMyFunction(args?.query ?? "");
      return res.json({ jsonrpc: "2.0", id, result });
    }
    return res.json({ jsonrpc: "2.0", id,
      error: { code: -32601, message: `Unknown tool: ${name}` } });
  }

  res.json({ jsonrpc: "2.0", id,
    error: { code: -32601, message: `Method not found: ${method}` } });
});

async function runMyFunction(query: string) {
  return { content: `Result for: ${query}`, artifacts: [], metadata: {} };
}

const SERVICE_URL = process.env.SERVICE_URL ?? "http://my-tool:8090";
const MANIFEST = {
  capability_id: "acme.tools.my-tool",
  namespace: "acme.tools",
  version: "1.0.0",
  kind: "remote_mcp",
  endpoint: `${SERVICE_URL}/mcp`,
  tools: [{ name: "my_function", description: TOOL_DEF.description,
            input_schema: TOOL_DEF.inputSchema }],
  tags: ["my-tag"],
  global: true,
  enabled: true,
};

async function registerWithRetry(retries = 10, delay = 3000) {
  const gateway = process.env.GATEWAY_URL!;
  const headers = { Authorization: `Bearer ${process.env.PLATFORM_ADMIN_TOKEN}` };

  for (let i = 1; i <= retries; i++) {
    try {
      const r = await axios.post(`${gateway}/admin/capabilities/register`,
                                 MANIFEST, { headers });
      console.log("[my-tool] registered:", r.data);
      return;
    } catch (e) {
      console.error(`[my-tool] attempt ${i}/${retries} failed:`, e.message);
      if (i < retries) await new Promise(r => setTimeout(r, delay));
    }
  }
  console.warn("[my-tool] WARNING: running unregistered");
}

app.listen(8090, async () => {
  console.log("[my-tool] listening on :8090");
  await registerWithRetry();
});
```

---

## Dockerfile template

```dockerfile
FROM python:3.12-slim          # or node:20-slim, etc.
WORKDIR /app
RUN pip install --no-cache-dir fastapi uvicorn httpx
COPY . .
EXPOSE 8090
CMD ["uvicorn", "main:app", "--host", "0.0.0.0", "--port", "8090"]
```

---

## docker-compose snippet

```yaml
services:
  my-tool:
    build:
      context: services/my-tool
    restart: unless-stopped
    environment:
      GATEWAY_URL: http://gateway:8080
      SERVICE_URL: http://my-tool:8090
      PLATFORM_ADMIN_TOKEN: "${PLATFORM_ADMIN_TOKEN}"
    expose: ["8090"]
    depends_on:
      gateway:
        condition: service_healthy
    deploy:
      resources:
        limits:
          memory: 256m
          cpus: "0.5"
```

Add the host to the gateway's allowlist in `docker-compose.yml`, e.g.
`CONUSAI_MCP_ALLOWED_HOSTS: current-time,my-tool:8090`. ConusAI Cloud's compose rules apply
(no host ports, numeric non-root user, explicit limits): see the header of
`docker-compose.yml`.

---

## Naming conventions

```
capability_id:  acme.billing.invoice-parser   ← reverse-DNS, unique, immutable
namespace:      acme.billing                  ← dot-separated group
tool.name:      parse_invoice                 ← snake_case, no dots, matches handler
capability:     acme.billing.parse_invoice    ← <namespace>.<tool.name>, what the agent sees
```

**Rules:**
- Keep `capability_id` stable across releases
- `tool.name` must match exactly what your `tools/call` handler checks; `<namespace>.<tool.name>` is unique in the registry
- Tool names containing dots are **rejected** — use underscores

---

## Authentication

`POST /admin/capabilities/register` always requires
`Authorization: Bearer <PLATFORM_ADMIN_TOKEN>`; the gateway refuses to start without that
variable, so there is no unauthenticated mode.

---

## What happens after registration

1. The gateway validates the manifest and checks `endpoint` against `CONUSAI_MCP_ALLOWED_HOSTS`.
2. Each tool is added to the in-memory capability registry as `<namespace>.<tool name>` — no restart.
3. The tool descriptions are embedded (fastembed) and upserted into Qdrant for semantic routing; the router cache is invalidated.
4. On the next agent turn, semantic routing can select your tool.

Registrations live in the gateway's memory: after a gateway restart every service must
register again, which the retry loop above does when the service itself restarts. While
the gateway keeps running, a second registration of the same names gets `409`.

---

## Debugging checklist

| Symptom | Check |
|---|---|
| Tool never selected by agent | Is description specific enough? Run a semantic search: `GET /v1/capabilities/search?q=your+query` |
| `Unknown tool: <name>` from MCP | The `tool.name` in the manifest doesn't match what your `tools/call` handler checks |
| `no provider for capability` | Dot-restore failed — ensure `tool.name` contains no dots |
| `401 Unauthorized` on register | Set `PLATFORM_ADMIN_TOKEN` env var on your service to match the gateway's token |
| `422` on register naming `endpoint` | The endpoint host is not in `CONUSAI_MCP_ALLOWED_HOSTS` |
| `422` naming `tenant_scope` | Set either `tenant_scope` (non-empty) or `"global": true`, not both |
| `409 Conflict` on register | The capability is already registered on this gateway process |
| Registration retries forever | Gateway isn't reachable — check `GATEWAY_URL` and that the gateway is healthy |
| Capability registered but not offered | `"enabled": false`, or disabled through `PATCH /admin/capabilities/{name}/enabled` |

---

## Manifest schema v2 fields (required for new capabilities)

All new capabilities must declare `schema_version = "2.0"` and the fields below.
Existing v1 manifests are still loaded but emit a deprecation warning at boot.

### Mandatory v2 fields

| Field | Type | Description |
|---|---|---|
| `schema_version` | string | `"2.0"` for new manifests. |
| `namespace` | string | Dot-separated slug from the taxonomy table, e.g. `"extract.fields.invoice"`. See [`docs/capabilities/taxonomy.md`](capabilities/taxonomy.md). |
| `category` | string | Taxonomy root: `storage`, `compute`, `sense`, `extract`, `convert`, `compose`, `deliver`, or `plan`. |

### Recommended v2 fields

| Field | Type | Default | Description |
|---|---|---|---|
| `accepts` | `[{mime, max_size_mb?}]` | `[]` | MIME globs this capability accepts. Required for `extract`, `convert`, `sense` categories. Powers the router's attachment post-filter. |
| `emits` | `[string]` | `[]` | MIME types produced as output artifacts. Used by planner for chaining suggestions. |
| `idempotent` | bool | `true` | Set `false` for write operations — prevents the planner from running them in `parallel_consensus`. |
| `cost_hint` | `{tokens?, dollars?, latency_ms?}` | `null` | Cost estimate for planner ranking. `dollars` takes precedence; falls back to `tokens` then `latency_ms`. |
| `requires` | `[string]` | `[]` | Capability names that must be registered for this capability to function. Gateway warns at load time if any are missing. |

### Taxonomy rule

Every capability namespace must be listed in [`docs/capabilities/taxonomy.md`](capabilities/taxonomy.md)
and `category` must match the namespace root. Running `cargo xtask capabilities lint`
enforces this — it must pass before merge.

### Full v2 TOML example

```toml
schema_version = "2.0"
name           = "extract-fields-invoice"
version        = "1.0.0"
namespace      = "extract.fields.invoice"
category       = "extract"
kind           = "chain"
description    = "Extract structured fields from invoice PDFs and images. Returns typed InvoiceData JSON. Call when the user uploads a bill, receipt, or purchase order."
tags           = ["invoice", "finance", "extraction", "vision"]
accepts        = [
    { mime = "application/pdf" },
    { mime = "image/*", max_size_mb = 20 },
]
emits          = ["application/json"]
idempotent     = true
cost_hint      = { dollars = 0.05, latency_ms = 4000 }
requires       = []
search_keywords = ["invoice", "bill", "receipt", "purchase order", "vendor invoice"]

[[tools]]
name        = "extract_invoice"
description = "Extract complete structured fields from an invoice image or PDF. Input: image_path. Output: InvoiceData JSON."

[tools.input_schema]
type     = "object"
required = ["image_path"]

[tools.input_schema.properties.image_path]
type        = "string"
description = "Local path or URL to invoice image or PDF"

[chain]
model           = "smart"
vision          = true
max_tokens      = 2048
system_prompt   = "You are an invoice data extraction specialist. Extract structured data with high accuracy. Always respond with valid JSON only."
prompt_template = "Extract all invoice information from the attached document. Return a valid JSON object with fields: invoice_number, invoice_date, due_date, issuer_name, issuer_address, billed_to_name, billed_to_company, currency, subtotal, tax_amount, total_amount, status, line_items. Use null for missing scalar fields, [] for missing arrays."
```

### Path template render context (`storage.*` capabilities)

Storage capabilities that use `[native] template = "..."` have access to these variables:

| Variable | Example value | Description |
|---|---|---|
| `{tenant_id}` | `acme-corp` | Current tenant ID |
| `{user_id}` | `alice` | Owner user ID (`__dev__` when absent) |
| `{thread_id}` | `thread-abc123` | Active chat thread ID (may be empty) |
| `{mime}` | `application/pdf` | Full MIME type of the uploaded file |
| `{mime_category}` | `application` | MIME category (the part before `/`) |
| `{original_name}` | `invoice.pdf` | Original filename (with extension) |
| `{now:%Y/%m/%d}` | `2026/05/20` | Current UTC date, strftime-formatted |

**Worked example per variable** — given tenant `acme-corp`, user `alice`, file `invoice.pdf` (`application/pdf`), uploaded on 2026-05-20:

| Template | Rendered result |
|---|---|
| `uploads/{tenant_id}/inbox` | `uploads/acme-corp/inbox` |
| `users/{user_id}/uploads` | `users/alice/uploads` |
| `{tenant_id}/{now:%Y/%m/%d}` | `acme-corp/2026/05/20` |
| `{mime_category}/{original_name}` | `application/invoice.pdf` |
| `{tenant_id}/{user_id}/{now:%Y-%m-%d}/{original_name}` | `acme-corp/alice/2026-05-20/invoice.pdf` |

The basic template engine supports only `{var}` and `{now:%fmt}` syntax. Complex Jinja2
expressions require `template_engine = "jinja"` (see taxonomy docs for details).

### Planner contract

When `plan.orchestrate` is installed, your capability may receive calls from the planner.
The planner sees your `name`, `description`, `accepts`, `emits`, and `cost_hint`. Write
descriptions from the planner's perspective: *"Call this when the input is a PDF and the
goal is to extract structured fields"* — not just *"processes documents"*.

---

## Real example: `current-time` service

Source: [`services/current-time/main.py`](../services/current-time/main.py)

```
namespace:     media.time
capability_id: media.time.current-time
tool name:     get_current_time
endpoint:      http://current-time:8082/mcp
```

The gateway's `docker-compose.yml` lists `current-time` in `CONUSAI_MCP_ALLOWED_HOSTS`.

This service:
- Exposes `POST /mcp` with `tools/list` and `tools/call`
- Registers itself on startup, retrying 25 times at 5-second intervals
- Handles `get_current_time(timezone?)` — returns an ISO 8601 timestamp
- Is selected by the agent when the user asks things like *"what time is it in Helsinki?"*
