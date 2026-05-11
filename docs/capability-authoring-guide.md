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
│  • Stores tool row in capability_specs (redb)           │
│  • Hot-reloads in-memory registry via tokio broadcast   │
│  • Embeds description with fastembed for semantic search │
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
  "name":          "invoice-parser",
  "namespace":     "acme.billing",
  "description":   "Parses invoices and extracts structured data.",
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
  "tenant_scope": [],
  "enabled":      true
}
```

#### Registration fields

| Field | Required | Description |
|---|---|---|
| `capability_id` | ✅ | Unique reverse-DNS ID: `"namespace.service-name"`. Used as the primary key. |
| `name` | ✅ | Short service name (no dots). Used in logs and the admin UI. |
| `namespace` | ✅ | Dot-separated namespace: `"acme.billing"`, `"media.time"`, etc. Groups related tools. |
| `description` | ✅ | Service-level description. |
| `version` | ✅ | Semantic version string. |
| `kind` | ✅ | Must be `"remote_mcp"` for self-registering services. |
| `endpoint` | ✅ | Full URL the gateway calls for `tools/list` and `tools/call`. |
| `tools` | ✅ | Array of tool definitions (see below). |
| `tags` | ✅ | Array of string tags for filtering and search. |
| `tenant_scope` | — | Array of tenant IDs. Empty `[]` = available to all tenants. |
| `enabled` | — | Defaults to `true`. Set `false` to register but not expose yet. |

#### Tool definition fields

| Field | Required | Description |
|---|---|---|
| `name` | ✅ | Function name. **No dots.** Underscores and hyphens OK. Must match what your `tools/call` handler expects. |
| `description` | ✅ | This is what the semantic router embeds and ranks. Write it from the user's perspective — describe *when to use this*, not just what it does. |
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
    token = os.environ.get("PLATFORM_ADMIN_TOKEN", "")
    headers = {"Authorization": f"Bearer {token}"} if token else {"X-Tenant-ID": "dev"}

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
| `GATEWAY_URL` | Base URL of the gateway, e.g. `http://agent-gateway:8080` |
| `PLATFORM_ADMIN_TOKEN` | Bearer token for registration auth. **Required in production.** Omit in local dev (no-auth mode). |
| `SERVICE_URL` | Your own service's public URL (used in `endpoint` in the manifest). |

#### Env-var migration (deprecated aliases)

Older versions of the reference service used `CONUSAI_PLATFORM_URL`, `CONUSAI_PLATFORM_TOKEN`, and `CONUSAI_SERVICE_URL`. These are still accepted for one release as fallbacks but emit a deprecation log line at startup. Switch to the canonical names above.

| Deprecated | Canonical replacement |
|---|---|
| `CONUSAI_PLATFORM_URL` | `GATEWAY_URL` |
| `CONUSAI_PLATFORM_TOKEN` | `PLATFORM_ADMIN_TOKEN` |
| `CONUSAI_SERVICE_URL` | `SERVICE_URL` |

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
    "name":          "my-tool",
    "namespace":     "acme.tools",
    "description":   "Does X for the user.",
    "version":       "1.0.0",
    "kind":          "remote_mcp",
    "endpoint":      f"{SERVICE_URL}/mcp",
    "tools":         [{"name": "my_function",
                       "description": TOOL_DEF["description"],
                       "input_schema": TOOL_DEF["inputSchema"]}],
    "tags":          ["my-tag"],
    "tenant_scope":  [],
    "enabled":       True,
}

async def register_with_retry(max_retries=10, delay=3.0):
    gateway = os.getenv("GATEWAY_URL", "http://agent-gateway:8080")
    token   = os.getenv("PLATFORM_ADMIN_TOKEN", "")
    headers = {"Authorization": f"Bearer {token}"} if token else {"X-Tenant-ID": "dev"}

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
  name: "my-tool",
  namespace: "acme.tools",
  description: "Does X for the user.",
  version: "1.0.0",
  kind: "remote_mcp",
  endpoint: `${SERVICE_URL}/mcp`,
  tools: [{ name: "my_function", description: TOOL_DEF.description,
            input_schema: TOOL_DEF.inputSchema }],
  tags: ["my-tag"],
  tenant_scope: [],
  enabled: true,
};

async function registerWithRetry(retries = 10, delay = 3000) {
  const gateway = process.env.GATEWAY_URL ?? "http://agent-gateway:8080";
  const token   = process.env.PLATFORM_ADMIN_TOKEN ?? "";
  const headers = token ? { Authorization: `Bearer ${token}` }
                        : { "X-Tenant-ID": "dev" };

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
    build: ./services/my-tool
    ports:
      - "8090:8090"
    environment:
      GATEWAY_URL:           http://agent-gateway:8080
      SERVICE_URL:           http://my-tool:8090
      PLATFORM_ADMIN_TOKEN:  ${PLATFORM_ADMIN_TOKEN:-}
    depends_on:
      - agent-gateway
```

---

## Naming conventions

```
capability_id:  acme.billing.invoice-parser   ← reverse-DNS, unique, immutable
namespace:      acme.billing                  ← dot-separated group
name:           invoice-parser                ← short service name, no dots
tool.name:      parse_invoice                 ← snake_case, no dots, matches handler
```

**Rules:**
- `capability_id` is the permanent primary key — do not change it after first deploy
- `tool.name` must match exactly what your `tools/call` handler checks; `(namespace, tool_name)` is unique in the DB
- Tool names containing dots are **rejected** — use underscores

---

## Authentication

| Environment | `PLATFORM_ADMIN_TOKEN` set? | Required header |
|---|---|---|
| Local dev | No | None (open) |
| Staging / Prod | Yes | `Authorization: Bearer <token>` |

---

## What happens after registration

1. Gateway writes one DB row per tool into `capability_specs(namespace, tool_name)`
2. Postgres fires a `NOTIFY capability_specs_changed` trigger
3. Gateway's realtime listener picks it up and hot-reloads the in-memory registry — **no restart**
4. fastembed embeds the tool description into `pgvector` for ANN search
5. On the next agent turn, semantic routing can select your tool

The whole flow from `POST /admin/capabilities/register` response to "tool available to agent" takes **under one second**.

---

## Debugging checklist

| Symptom | Check |
|---|---|
| Tool never selected by agent | Is description specific enough? Run a semantic search: `GET /v1/capabilities/search?q=your+query` |
| `Unknown tool: <name>` from MCP | The `tool.name` in the manifest doesn't match what your `tools/call` handler checks |
| `no provider for capability` | Dot-restore failed — ensure `tool.name` contains no dots |
| `401 Unauthorized` on register | Set `PLATFORM_ADMIN_TOKEN` env var on your service to match the gateway's token |
| Registration retries forever | Gateway isn't reachable — check `GATEWAY_URL` and Docker network |
| Capability registered but disabled in UI | `"enabled": true` missing from manifest, or disabled manually via admin UI |

---

## Real example: `current-time` service

Source: [`services/current-time/main.py`](../services/current-time/main.py)

```
namespace:     media.time
capability_id: media.time.current-time
tool name:     get_current_time
endpoint:      http://current-time:8082/mcp
```

This service:
- Exposes `POST /mcp` with `tools/list` and `tools/call`
- Registers itself on startup with 10-retry backoff
- Handles `get_current_time(timezone?)` — returns an ISO 8601 timestamp
- Is selected by the agent when the user asks things like *"what time is it in Helsinki?"*
