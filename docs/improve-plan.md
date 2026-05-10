# Improve Plan — Generic, Prompt-Driven Capabilities

**Goal:** make every capability look and behave the same to the UI and the agent loop. All domain logic (what to extract, how to validate, how to summarise) moves into the **capability manifest's prompt + JSON schema**. Rust code stops being a per-domain hand-coded pipeline.

The UI must not know that "invoice" exists. The backend must not have a `/ui/extract-invoice` bypass. The agent picks the right tool from manifest descriptions; the UI renders every tool result through one component.

---

## 1. Current state — where the abstraction leaks

Audited 2026-05-10. Reference `git grep` paths below.

### Frontend leaks (`apps/web/src/`)

| File | Lines | Leak |
|---|---|---|
| `lib/api/endpoints.ts` | 13 | Hardcoded `UI_EXTRACT_INVOICE: '/ui/extract-invoice'` |
| `lib/api/types.ts` | 7–13 | `InvoiceData` interface with invoice-specific fields |
| `routes/+page.svelte` | 412–433 | `extractInvoice()` calls bypass endpoint |
| `routes/+page.svelte` | 476–478 | `INVOICE_EXT`, `INVOICE_NAME`, `isInvoice()` heuristics |
| `routes/+page.svelte` | 488 | `handleSubmit` routes attachments based on invoice detection |
| `routes/+page.svelte` | ~742–746 | `__invoice__` message marker + special branch |
| `routes/+page.svelte` | ~867–916 | `invoiceCard()` snippet — bespoke layout (`inv-badge`, `inv-table`, etc.) |

### Backend leaks (`apps/backend/`)

| File | Leak |
|---|---|
| `crates/agent-gateway/src/ui/handlers/invoice.rs` | Whole handler bypasses agent loop, instantiates `InvoicePipeline` directly |
| `crates/agent-gateway/src/ui/routes.rs:40` | `/ui/extract-invoice` route registration |
| `crates/agent-core/src/tools/providers/chain.rs:19–84` | `InvoiceProvider::invoke()` `match` on `"extract_invoice"` → `InvoicePipeline` |
| `crates/agent-core/src/chains/invoice.rs` | Rust-coded vision + JSON-schema extraction (per-domain) |
| `crates/agent-core/src/chains/contract.rs` | Same pattern for contracts |
| `crates/agent-gateway/src/main.rs` | `TranscribeVideoCapability` registered with synthetic in-process manifest |

### What's already generic (build on this)

- **Registry**: `agent-core/src/tools/registry.rs` — pluggable factories (`McpFactory`, `WasmFactory`, `ChainFactory`, `BuiltinFactory`, **`DynamicPromptFactory`**).
- **Provider trait**: `agent-core/src/tools/provider.rs` — single `invoke()` contract.
- **Agent loop**: `agent-gateway/src/routes/agent.rs` — picks tools from registry by LLM tool-call.
- **SSE events**: `tool_call_start { id, name }` and `tool_call_result { tool_use_id, name, result }` — capability-agnostic.
- **DynamicPromptCapability**: `agent-core/src/chains/dynamic_prompt.rs` — DB-backed, versioned, prompt-driven chains. **This is the model for everything.**
- **Generic `tool-card`** in `+page.svelte:774–787` — already renders any tool's status/name/duration.

---

## 2. Target architecture — one path for every capability

```
┌────────────┐  upload chip    ┌──────────────┐  POST /ui/stream    ┌──────────────────┐
│  Frontend  │ ───────────────▶│   Gateway    │ ──────────────────▶ │  Agent loop      │
│            │                 │              │   (SSE)             │  + tool registry │
│  one chat  │◀─tool_card SSE──│              │◀─────────────────── │                  │
│  one card  │                 └──────────────┘                     └────────┬─────────┘
└────────────┘                                                                │
                                                                              ▼
                                                              ┌────────────────────────────┐
                                                              │ Capability (prompt-driven) │
                                                              │  • system_prompt (TOML)    │
                                                              │  • user_template (TOML)    │
                                                              │  • output_schema (TOML)    │
                                                              │  • result_view  (TOML)     │
                                                              └────────────────────────────┘
```

**Rules of the new world:**

1. The frontend never names a capability. No regex on filenames, no per-capability buttons, no per-capability types.
2. The backend has **no** `/ui/<capability>-<verb>` shortcut routes. Everything flows through `/ui/stream`.
3. A capability is fully described by `capability.toml` — including the prompt that does the domain work and the schema that describes the output.
4. The UI renders any tool result from `output_schema` + an optional `result_view` template declared by the manifest. No Svelte code knows about invoices, contracts, or transcripts.

---

## 3. Manifest schema additions

Extend `capability.toml` so domain logic and rendering hints live there:

```toml
name = "invoice-processing"
version = "2.0.0"
description = "..."
kind = "prompt"           # NEW: replaces "chain" for prompt-driven capabilities
tags = ["invoice", "finance", "vision", "structured-extraction"]

[[tools]]
name = "extract_invoice"
description = "Extract structured fields from an invoice image or PDF."

# When the LLM should reach for this tool — moves heuristics out of UI
[tools.intent_hints]
file_extensions = ["png", "jpg", "jpeg", "pdf"]
filename_patterns = ["invoice", "receipt", "bill", "facture"]
keywords = ["extract", "parse", "what's the total"]

# What goes in to the chain
[tools.input_schema] # JSON Schema (already present)
type = "object"
required = ["image_path"]
properties.image_path = { type = "string" }

# Domain logic — what was hand-coded in InvoicePipeline.rs is now PROMPTS
[tools.prompt]
system = """
You are an expert invoice-data extractor. Given an image or PDF of an invoice,
return a JSON object that strictly matches the output_schema. Numbers must be
machine-parseable. Preserve original currency codes. If a field is not visible,
use null — never guess.
"""
user_template = """
Extract every field from the attached invoice.
File: {{ input.image_path }}

Return ONLY the JSON. No prose, no markdown fences.
"""
model = "claude-opus-4-7"
max_tokens = 2048
vision = true              # Tells the chain executor to load image bytes

# What the LLM must produce — drives generic UI rendering
[tools.output_schema]
type = "object"
properties = { invoice_number = {type = "string"}, total_amount = {type = "number"}, ... }

# Optional: how the UI should display the result
[tools.result_view]
kind = "card"              # one of: "card" | "table" | "raw_json" | "markdown"
title_field = "invoice_number"
badge_field = "status"
sections = [
  { label = "Parties", fields = ["issuer_name", "billed_to_name"] },
  { label = "Line items", table_field = "line_items" },
  { label = "Totals", fields = ["subtotal", "tax_amount", "total_amount"] },
]
```

**Key addition: `result_view`** is *display metadata*, not domain logic. It tells the **generic** UI card how to lay out the JSON the prompt produced. Any capability can adopt the same five view kinds.

---

## 4. Refactor steps (ordered, mergeable independently)

### Phase 1 — Backend: prompt-driven capabilities behind the existing trait

**1.1 Define new manifest fields** (`agent-core/src/tools/manifest.rs`)
- Add `intent_hints`, `prompt` (or extend existing `LlmChainConfig`), `output_schema`, `result_view` fields. All optional for backwards compatibility.

**1.2 Extend `ChainFactory` to support `kind = "prompt"`**
- Currently `chain` kind hardcodes pipeline names; add a generic prompt-driven path that uses `LlmChainConfig` from the manifest the same way `DynamicPromptCapability` does — but loading from TOML instead of DB.
- Reuse `chains/executor.rs` (already prompt-driven).

**1.3 Port `invoice-processing` and `contract-processing` to `kind = "prompt"`**
- Move the system-prompt + JSON-schema text from `chains/invoice.rs` and `chains/contract.rs` into their `capability.toml`.
- Delete `chains/invoice.rs` and `chains/contract.rs`. Delete the `match` branches in `tools/providers/chain.rs:46–84`.
- Smoke test: `extract_invoice` still works through the agent loop with identical output.

**1.4 Delete the bypass route**
- Remove `ui/handlers/invoice.rs` entirely.
- Remove route registration in `ui/routes.rs:40`.
- Remove `EP.UI_EXTRACT_INVOICE` from frontend (Phase 3).

**1.5 Generic file-attachment context**
- When `/ui/stream` receives `attachments: [{token, filename, content_type}]`, inject them into the LLM context as a system message: *"User has attached files: invoice.png (image/png) at /ui/files/<token>."*
- The LLM uses this + capability `intent_hints` to pick the right tool. No filename regex anywhere.

### Phase 2 — Generic UI rendering

**2.1 Add `result_view` payload to SSE events**
- Extend `tool_call_result` SSE to include `{ schema, view }` from manifest, so the frontend doesn't need a manifest lookup. Stays a pure data event.
- Or: add `GET /v1/capabilities/<name>/tools/<tool>/view` for lazy fetch + cache.

**2.2 Build one `<ToolResultCard>` Svelte component**
- Inputs: `{ name, result, view, schema }`.
- Branches on `view.kind`:
  - `card` — title + badge + sections (fields + nested tables)
  - `table` — JSON-array-of-objects → `<table>` with column inference
  - `markdown` — render result.markdown via existing markdown renderer
  - `raw_json` — `<pre><code>` fallback
- All styling shared (no per-capability CSS).

**2.3 Replace `invoiceCard` and the `__invoice__` marker**
- Tool results stream in as `tool_call_result` events. The chat already renders these as tool cards (lines 774–787). Extend that path to render `<ToolResultCard>` instead of the current minimal status pill **after** the result arrives.
- Delete `invoiceCard` snippet, `__invoice__` marker handling, `InvoiceData` type, all `inv-*` CSS classes.

### Phase 3 — Frontend: remove all capability-specific code

**3.1 Delete from `+page.svelte`:**
- `extractInvoice()` function (412–433)
- `INVOICE_EXT`, `INVOICE_NAME`, `isInvoice()` (476–478)
- The `isExtractIntent` branch in `handleSubmit` (added 2026-05-10) — no longer needed
- "Extract invoice" button on the chip (~822–826)
- `__invoice__` message branch (~742)
- `invoiceCard` snippet (~867–916) and its CSS

**3.2 Delete from `lib/api/`:**
- `EP.UI_EXTRACT_INVOICE`
- `InvoiceData` interface
- Any invoice-named helpers

**3.3 New attachment flow**
- Upload → chip with filename + size only (already this minus the button).
- Submit → `/ui/stream` with `{ message, attachments: [{token, filename, content_type}] }`. Backend constructs the LLM prompt; agent picks tool via `intent_hints`.
- `tool_call_start` shows a generic spinner card with the capability name. `tool_call_result` swaps it for the rendered `<ToolResultCard>`.

### Phase 4 — Cleanup, tests, docs

- **Tests:** add a parametric integration test that uploads a fixture file for each capability and asserts the agent picks the right tool by description + `intent_hints` alone (no UI hints).
- **Docs:** update `docs/capability-authoring-guide.md` to teach prompt-driven capabilities and the new `result_view` block. Add a worked example.
- **Migrations:** existing DB-stored prompts in `dynamic_prompts` table keep working — `DynamicPromptCapability` already uses the same `LlmChainConfig`. Optionally seed manifests from DB on first boot for parity.
- **Telemetry:** keep the existing `tool_call_start/result` audit events — they already log capability + tool name generically.

---

## 5. Acceptance criteria

A new contributor adds a capability `recipe-extract` by **only**:

1. Writing `apps/backend/capabilities/recipe-extract/capability.toml` (manifest + system prompt + output_schema + result_view).
2. Restarting the gateway.

…and gets:

- A capability sidebar entry (already automatic).
- Correct tool selection when the user uploads `dinner.jpg` and types "what ingredients?" — driven by the manifest's `intent_hints` and `description`, not UI heuristics.
- A rendered structured card with their fields — through the same `<ToolResultCard>` that renders invoices, contracts, OCR output, etc.
- Zero changes anywhere in `apps/web/` or `apps/backend/crates/`.

If any of those four require code changes, the abstraction is still leaking and the refactor isn't done.

---

## 6. Risks and trade-offs

| Risk | Mitigation |
|---|---|
| Pure-prompt extraction is sometimes lower quality than hand-tuned Rust pipelines | Keep `kind = "chain"` available for capabilities that genuinely need bespoke code (e.g. OCR-then-prompt-then-validate). Default new capabilities to `prompt`. |
| Latency / cost increase from going through full agent loop instead of bypass | Allow capabilities to declare `direct_invoke_allowed = true` in manifest; surface a generic `/ui/run/<capability>/<tool>` endpoint that any capability can use as a power-user shortcut. **Generic, not per-capability.** |
| Generic renderer can't match the polish of bespoke cards | The five view kinds (`card`, `table`, `markdown`, `raw_json`, future `chart`) cover ~95% of structured outputs. Visual polish goes into the shared component, lifting all capabilities at once. |
| Migration breaks existing flows | Phases 1 and 2 ship behind a feature flag; old `/ui/extract-invoice` stays until UI is migrated; remove only after Phase 3. |

---

## 7. Out of scope for this plan

- MCP capabilities (`google-workspace`) already follow a generic provider pattern — they need the same `result_view` treatment but no prompt rework.
- WASM capabilities — same: they emit JSON; just need `output_schema` + `result_view` declared.
- Auth, RBAC, multi-tenancy — unchanged.
- Workspace tree, RECENTS, super-admin UI — unchanged.

---

## 8. Suggested ordering and effort

| Phase | Effort | Ships independently? |
|---|---|---|
| 1.1–1.2 manifest + `kind = "prompt"` factory | M | yes (no UI change) |
| 1.3 port invoice/contract to prompt manifests | M | yes (behaviour-equivalent) |
| 1.4 delete bypass route | S | no — depends on 3.1 |
| 2.1–2.2 SSE + `<ToolResultCard>` | M | yes (renders in parallel with old card) |
| 2.3 replace invoice marker | S | depends on 2.2 |
| 3.x frontend cleanup | S | depends on 1.4 + 2.3 |
| 4 tests + docs | S | rolling |

Total: ~1–2 weeks of focused work, fully decomposable into reviewable PRs.
