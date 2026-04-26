# ConusAI Platform — Implementation Plan

*Rust 1.88 · Rig 0.9 · Anthropic Claude · Qdrant · MinIO · WASM · OpenTelemetry — April 2026*

Status legend: ✅ Done · 🔄 In progress · ⬜ Planned

---

## Phase 0 — Project Initialization ✅

- Cargo workspace (`common`, `agent-core`, `agent-gateway`, `invoice-demo`, `evals`)
- Folder structure: `capabilities/`, `evals/datasets/`, `wasm/`, `scripts/`, `docs/`
- `.env.example`, `.gitignore`, `start.sh`, `docker-compose.yml`, `rust-toolchain.toml`

---

## Phase 1 — Core Foundation (`common` crate) ✅

- `AppConfig` / `ServerConfig` / `QdrantConfig` / `TelemetryConfig` via `figment`
- `ConusAiError` + `ApiError`
- `TelemetryGuard` — JSON tracing + optional OTLP/Jaeger
- `HttpClient`, `JsonRpcRequest/Response`, `WasmLoader`
- `ThreadStore` trait + `Thread` / `Message` / `ToolCall` types
- `safe_join` / `join_under_tenant` path safety
- Constants in `limits.rs`
- **19 tests passing**

---

## Phase 2 — Agent Core (`agent-core` crate) ✅

- `CapabilityManifest` / `CapabilityKind` / `ToolDef` — YAML parsing
- `CapabilityCard`, `CapabilityRegistry` (auto-discovery from `capabilities/`)
- `CapabilityExecutor` — routes `pipeline` → `InvoicePipeline`, `wasm` → `WasmCapabilityLoader`, `mcp` → `McpAdapter`, `native` → `tools/`
- `GeneralAgentBuilder` / `AgentRuntime` (Rig 0.9 Anthropic)
- `TenantContext` / `PlanTier` (Free/Pro/Enterprise, rate limits, path scoping)
- `InvoicePipeline` — Claude vision + strict JSON schema → `InvoiceData` (20+ fields)
- `QdrantThreadStore` — Qdrant as document store, one collection per tenant, auto-summarisation
- Native tools: `read_file`, `write_file`, `run_cargo` (allowlisted subcommands)
- WASM execution via Wasmtime (component model, `wasm32-wasip1`)

---

## Phase 3 — Capabilities System ✅

Capabilities are zero-code: drop a `capability.yaml` folder into `capabilities/` and the registry auto-discovers it.

| Capability | Kind | Tools | Status |
|---|---|---|---|
| `file-storage` | mcp | `upload_file`, `download_file`, `presigned_url` | ✅ |
| `ocr-service` | pipeline | `extract_text` | ✅ |
| `invoice-processing` | pipeline | `extract_invoice`, `validate_invoice` | ✅ |
| `google-workspace` | mcp | `list_files`, `read_document`, `append_to_sheet`, `send_email` | ✅ manifest |
| `template-wasm` | wasm | `ping` | ✅ |
| `native-tools` | native | `read_file`, `write_file`, `run_cargo` | ✅ |

### Capability selection (updated 2026-04-26)

`ocr-service` and `invoice-processing` are intentionally non-overlapping:

| Document type | Use |
|---|---|
| Invoice, bill, purchase order | `invoice-processing__extract_invoice` — handles vision internally, returns typed `InvoiceData` |
| Contract, letter, handwritten note, generic document | `ocr-service__extract_text` — returns raw plain text |

Tool routing is description-driven: rich `description` fields in `capability.yaml` are loaded verbatim into Anthropic tool definitions at startup. No code-level classifier needed.

**Verified live (2026-04-26):** Agent correctly chose `file-storage__download_file` → `invoice-processing__extract_invoice` → `file-storage__presigned_url` for `invoice.png` without redundant `ocr-service` call. Extracted full `InvoiceData` for Hostinger invoice `HCY-23256029` (€63.99 EUR, PAID).

---

## Phase 4 — Evals Framework ✅

- `evals` binary — `run --suite invoice|threads|ocr_quality`, `list`
- `InvoiceScorer` — field-level accuracy (string match + `abs(diff) < 0.01` for amounts), threshold 0.8
- `threads` runner — multi-turn recall via gateway
- `ocr_quality` runner — snippet presence scoring
- Datasets: `evals/datasets/{invoice,threads,ocr_quality}.jsonl`

---

## Phase 5 — Agent Gateway ✅

- OpenAI-compatible `POST /v1/chat/completions` (SSE + blocking)
- `POST /v1/agent/completions` — thread-aware tool-calling loop (≤5 rounds), SSE streaming with `tool_call_start` / `tool_call_result` events
- Thread REST API: `POST /v1/threads`, `GET /v1/threads`, `GET /v1/threads/{id}`, `GET /v1/threads/{id}/messages`, `POST /v1/threads/{id}/messages`
- `GET /v1/capabilities`, `GET /v1/capabilities/search?q=`
- `POST /mcp` — JSON-RPC 2.0 dispatcher
- `POST /v1/files`, `GET /v1/files/{token}` — MinIO upload + token-gated download
- JWT auth (HS256) in production; `X-Tenant-ID` dev fallback
- Per-tenant rate limiting (sliding 60s window, plan-based RPM)
- W3C traceparent propagation, `gen_ai.*` OTel span attributes

---

## Phase 6 — Infrastructure ✅

- `docker-compose.yml` profiles: `infra` (Qdrant + MinIO), `observability` (Jaeger + OTel), `full`
- 4-stage `cargo-chef` Dockerfile (10× faster incremental builds)
- `start.sh` orchestration: brings up infra, polls healthchecks, builds gateway, prints summary URLs
- MinIO bucket `conusai` auto-initialized via `minio-init` service
- Qdrant collections per tenant (threads, capabilities)
- Healthchecks at every layer

---

## Phase 7 — Foundry UI ✅

Full browser-based chat interface served by `agent-gateway` at `/`.

- **Auth:** HMAC-signed session cookie (`conusai_session`, HttpOnly, SameSite=Lax)
- **Streaming:** SSE via `POST /ui/stream` → in-process `agent::stream_agent` (no HTTP self-call)
- **Tool cards:** `<details>` with status dot, timing, collapsible JSON — rendered live during stream
- **File upload:** `POST /ui/upload` → MinIO; attachment chips; drag-drop on composer
- **Recents:** sidebar thread list; click → load history from `/v1/threads/{id}/messages`
- **Theme:** dark/light toggle with localStorage, CSS custom properties
- **Mobile:** fixed sidebar drawer, backdrop overlay, hamburger toggle
- **Keyboard:** `⌘K` focus, `⌘N` new chat, `⌘/` theme, `Esc` blur
- **Toasts:** success/error notifications via `window.__toast`
- **Fonts:** Askama compile-time templates; assets served from `CONUSAI_UI_ASSETS`

---

## Phase 8 — Polish, Observability & Quality ✅

- Structured JSON logs + OTLP → Jaeger (Jaeger UI at `:16686`)
- OTel Collector config (`scripts/otel-collector.yaml`)
- `gen_ai.*` semantic conventions on agent spans
- 19 unit tests (`cargo test --workspace`)
- `cargo clippy --workspace -- -D warnings` clean
- End-to-end verified: invoice upload → extraction → structured output

---

## Planned / Next

| Item | Priority | Notes |
|---|---|---|
| `Docker` capability kind | Low | Reserved in `CapabilityKind` enum |
| External MCP server federation | Medium | `McpAdapter` exists, needs routing layer |
| `contract-processing` capability | Medium | Follow `invoice-processing` YAML pattern |
| Persistent audit log | Medium | Append-only log per tenant |
| Billing / quota enforcement | Low | `PlanTier` already has limits |
| Admin dashboard | Low | Capability management UI |
| CI/CD GitHub Actions | Medium | build + test + evals workflow |
| Multi-instance deployment | Low | Qdrant + MinIO already stateless-ready |
