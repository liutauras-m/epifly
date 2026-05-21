# Capabilities — Architecture & Functional Reference

> **Scope.** This document describes the capability subsystem of the ConusAI
> platform end-to-end: the manifest schema, the runtime that loads and
> dispatches capabilities, the seven capability *kinds*, the on-disk inventory
> shipped in [`apps/backend/capabilities`](../../apps/backend/capabilities/),
> and the out-of-process self-registering pattern demonstrated by the
> [`current-time`](../../services/current-time/) container.
>
> Companion docs: [capabilities-arch overview](capabilities-arch.md) (this file),
> [taxonomy.md](taxonomy.md), [orchestration.md](orchestration.md),
> [upload-pipeline.md](upload-pipeline.md), [how-to-add-a-domain.md](how-to-add-a-domain.md),
> [ADR-0007 — Everything is a Capability](../adr/0007-everything-is-a-capability.md).

---

## 1. Big picture

> **One sentence:** A *capability* is a uniformly-described unit of work
> (an LLM chain, a WASM module, an in-process Rust function, or a remote
> MCP service) that the agent loop can semantically discover, validate,
> invoke, and observe through a single registry.

```
┌─────────────────────────────────────────────────────────────────────┐
│                            Agent Loop                                │
│   user msg → SemanticCapabilityRouter.tool_definitions(msg, hint)   │
│           → LLM picks tool → ToolExecutor.invoke(cap, tool, input)  │
└──────────────────┬──────────────────────────────────────────────────┘
                   │
        ┌──────────▼──────────┐         ┌───────────────────────────┐
        │ CapabilityRegistry  │◄────────│  CapabilityDiscovery      │
        │  (in-memory cards)  │         │  + ManifestWatcher        │
        └──────────┬──────────┘         │  (load *.toml + hot-reload)│
                   │                    └───────────────────────────┘
        ┌──────────▼──────────┐         ┌───────────────────────────┐
        │ CapabilityProvider  │◄────────│  CapabilityAdmin          │
        │  trait (per-kind)   │         │  + POST /admin/.../register│
        └──────────┬──────────┘         │  (DB-backed dynamic regs) │
                   │                    └───────────────────────────┘
   ┌───────┬───────┼───────┬───────────┬────────────┬───────────────┐
   │ chain │ wasm  │ mcp   │ remote_mcp│ native     │ dynamic_prompt│
   │       │       │ (file)│ (JSON reg)│ (built-in) │ (DB row)      │
   └───────┴───────┴───────┴───────────┴────────────┴───────────────┘
```

The system is built around three north-star invariants:

1. **Everything is a capability.** Document extraction, file storage, OCR,
   planning, even debug ping — all expressed as capabilities behind the same
   `CapabilityProvider` trait. See
   [ADR-0007](../adr/0007-everything-is-a-capability.md).
2. **Never send the full catalogue to the LLM.** The
   [`SemanticCapabilityRouter`](../../apps/backend/crates/agent-core/src/capabilities/semantic_router.rs)
   ANN-prefilters to top-K (≤ 50) per turn against a Qdrant vector index
   built from each manifest's `embedding_text()`.
3. **All LLM work flows through `LlmRegistry`.** Capability code is forbidden
   from instantiating provider SDKs directly (enforced at compile time by
   `agent-core`'s `build.rs`).

---

## 2. Manifest schema (`capability.toml`, schema_version 2.0)

Each capability lives in its own directory under
`apps/backend/capabilities/<name>/` and is declared by a single
`capability.toml`. The Rust type is
[`ToolManifest`](../../apps/backend/crates/agent-core/src/capabilities/manifest.rs).

### 2.1 Top-level fields

| Field             | Type                          | Purpose                                                                                                 |
| ----------------- | ----------------------------- | ------------------------------------------------------------------------------------------------------- |
| `schema_version`  | `"1.0"` \| `"2.0"`            | Loader accepts both; new capabilities use 2.0.                                                          |
| `name`            | string (kebab-case)           | Unique registry key, e.g. `"invoice-processing"`.                                                       |
| `version`         | semver string                 | Manifest version (informational; bump on breaking input/output changes).                                |
| `namespace`       | dot-slug                      | Routing identity, e.g. `"extract.fields.invoice"`. See [taxonomy.md](taxonomy.md).                       |
| `category`        | taxonomy root                 | Coarse bucket: `extract` / `convert` / `compose` / `sense` / `plan` / `storage` / `deliver` / `compute`. |
| `kind`            | enum (see §3)                 | Selects the `CapabilityFactory` that builds the provider.                                                |
| `description`     | string                        | Sent to the LLM verbatim — write it like a tool docstring.                                              |
| `tags`            | string array                  | Free-form labels; included in the embedding text.                                                       |
| `search_keywords` | string array                  | Synonyms / example queries — *embedding-only*, never shown to the model.                                |
| `accepts`         | array of `AcceptSpec`         | MIME post-filter for the router. Bare string or `{ mime, max_size_mb }`.                                 |
| `emits`           | string array                  | MIME types this capability may produce.                                                                  |
| `idempotent`      | bool (default `true`)         | Whether the executor may retry / fan out in `parallel_consensus`.                                        |
| `cost_hint`       | bucket label \| object        | `"low"` / `"medium"` / `"high"` *or* `{ dollars, latency_ms, tokens }`. Used by ranking & UI.            |
| `requires`        | string array                  | Names of other capabilities that must be registered; router warns at load.                              |
| `tenant_scope`    | string array                  | Empty = global. Non-empty = visible only to the listed tenant IDs.                                       |
| `enabled`         | bool (default `true`)         | Set to `false` to disable without removing the directory.                                                |

### 2.2 `[[tools]]` blocks

Each capability exposes one or more *tools* (the unit the LLM actually calls).
Anthropic constrains tool names to `^[a-zA-Z0-9_-]{1,128}$`, so the executor
joins them as `{cap_name_with_dots_replaced_by_underscores}__{tool_name}`
(see `tool_definitions_from_manifest()` in
[`executor.rs`](../../apps/backend/crates/agent-core/src/capabilities/executor.rs)).

```toml
[[tools]]
name = "extract_invoice"
description = "Extract complete structured fields from an invoice…"

[tools.input_schema]
type = "object"
required = ["image_path"]

[tools.input_schema.properties.image_path]
type = "string"
description = "Absolute local path or http/https URL to the invoice"
```

### 2.3 `[chain]` block (only for `kind = "chain"`)

Declares the data-driven LLM pipeline — no Rust code required.

| Field             | Purpose                                                                                                  |
| ----------------- | -------------------------------------------------------------------------------------------------------- |
| `model`           | Alias (`"smart"`, `"fast"`) or concrete id (`"claude-opus-4-7"`).                                        |
| `system_prompt`   | Role / persona for the call.                                                                              |
| `prompt_template` | Mustache-style template with `{{input.field}}`, `{{tenant.id}}`, `{{#if input.foo}}…{{/if}}` supported.  |
| `vision`          | When `true`, executor reads `input.image_path` and sends base64 image bytes alongside the prompt.        |
| `max_tokens`      | Generation cap (default 2048).                                                                            |
| `output_schema`   | Optional JSON Schema — response is parsed and validated; mismatch returns a typed error to the agent.    |

### 2.4 `[config]` block

A free-form `serde_json::Value` consumed by the per-kind factory:

- **`native`** — `op = "<dispatch key>"`; some natives also embed
  `[[config.steps]]` (e.g. [`plan-on-upload/capability.toml`](../../apps/backend/capabilities/plan-on-upload/capability.toml)).
- **`wasm`** — `wasm_module = "tesseract_ocr.wasm"`.
- **`mcp`** — adapter-specific (endpoint, OAuth scopes, env-var refs like `${S3_ENDPOINT}`).

---

## 3. Capability kinds

`ToolKind` enumerates seven runtime strategies. Each has a dedicated
`CapabilityFactory` in
[`crates/agent-core/src/capabilities/providers/`](../../apps/backend/crates/agent-core/src/capabilities/providers/).

| Kind             | Source of truth          | Provider                                | Typical use                                                                  |
| ---------------- | ------------------------ | --------------------------------------- | ---------------------------------------------------------------------------- |
| `chain`          | `[chain]` block in TOML  | `PromptChainCapability` via `ChainFactory` | Data-driven LLM tools (extraction, classification, composition, planning).   |
| `wasm`           | `*.wasm` in cap dir      | `WasmCapability` (`WasmFactory`)        | Deterministic, offline, low-latency compute (MIME magic-byte sniff, Tesseract). |
| `mcp`            | TOML manifest + endpoint | `McpProvider`                           | File-based registration of an MCP server (S3, Google Workspace).             |
| `remote_mcp`     | DB row from admin API    | `RemoteMcpCapability`                   | Out-of-process services that self-register at startup (e.g. `current-time`). |
| `native`         | Built-in Rust function   | Domain-specific provider in `agent-core`/`agent-gateway` | Storage primitives, job-backed transcription, planner policies.              |
| `dynamic_prompt` | DB row (versioned prompt)| `DynamicPromptCapability`               | DB-managed prompt engineering with rollback — no recompile.                  |
| `docker`         | TOML manifest            | *(reserved; not currently wired)*       | Future: container-per-invocation isolation.                                  |

### 3.1 How a kind is resolved

1. `CapabilityRegistry::load_from_dir(dir)` reads every `capability.toml`,
   builds a `CapabilityCard` (manifest + path + provider slot + audit state).
2. `factory_for(card)` walks `factories` until one returns `supports(kind, name) == true`.
3. `factory.create(card)` produces an `Arc<dyn CapabilityProvider>` stored on the card.
4. Errors are captured on `card.last_error` — the card remains visible to admins
   but disabled.

### 3.2 The `CapabilityProvider` contract

```rust
#[async_trait]
pub trait CapabilityProvider: Send + Sync {
    fn manifest(&self) -> &ToolManifest;
    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value>;
    /// Default impl reads from the manifest; providers may override.
    fn tool_definitions(&self) -> Vec<Value> { tool_definitions_from_manifest(self.manifest()) }
}
```

Every kind boils down to this interface. The agent loop never branches on kind.

---

## 4. Runtime subsystems

All paths in this section are inside
[`apps/backend/crates/agent-core/src/capabilities/`](../../apps/backend/crates/agent-core/src/capabilities/).

### 4.1 `CapabilityRegistry` ([`registry.rs`](../../apps/backend/crates/agent-core/src/capabilities/registry.rs))

In-memory index keyed by manifest `name`. Stores:

- `cards: HashMap<String, CapabilityCard>` — manifest, provider, enabled flag, `last_error`, timestamps.
- `factories: Vec<Box<dyn CapabilityFactory>>` — one per kind (Mcp, Wasm, Chain, optionally DynamicPrompt).
- `bulk_factories: Vec<Box<dyn BulkCapabilityFactory>>` — used at boot for DB-backed (`remote_mcp`, `dynamic_prompt`) populations.
- `namespace_index: HashMap<String, Vec<String>>` — child segments for admin autocomplete.

Mutators (`register`, `replace`, `unregister`, `set_enabled`, `reload_capability`)
keep the namespace index in sync. Readers (`get`, `enabled_for_tenant`,
`search_by_namespace`, `search_by_tag`) are lock-free.

### 4.2 Discovery & hot-reload ([`discovery.rs`](../../apps/backend/crates/agent-core/src/capabilities/discovery.rs))

`CapabilityDiscovery::from_env()` reads `CONUSAI_CAPABILITIES_DIR` (default
`./capabilities`). In the gateway container this is mounted to
`/app/capabilities` (see [docker-compose.yml](../../docker-compose.yml#L189) —
`./apps/backend/capabilities:/app/capabilities`).

`ManifestWatcher` uses `notify` to debounce (250 ms) filesystem events; on
change it calls `Registry::reload_capability(dir)` and emits a
`capability.reloaded` realtime event on the `__system__` channel so connected
UIs can refresh. Editing a `capability.toml` is therefore a hot-deploy.

### 4.3 Semantic router ([`semantic_router.rs`](../../apps/backend/crates/agent-core/src/capabilities/semantic_router.rs))

Per-turn pipeline:

1. **Embedding text** — `ToolManifest::embedding_text()` concatenates name,
   description, tags, tool docs, `search_keywords`, plus enrichment tokens
   `CATEGORY:<cat>`, `MIME:<mime>`, `EMITS:<mime>`, `COST:<bucket>` to widen
   ANN recall.
2. **Index** — vectors land in Qdrant (`conusai-qdrant`, port `6334` gRPC).
3. **Query** — embed user message + optional `AttachmentHint { mimes, cost_bias }`,
   return top-K cards.
4. **Post-filter** — drop hits whose `accepts` doesn't match any hint MIME
   (empty `accepts` = pass).
5. **Tenant gate** — drop hits where `tenant_scope` excludes the caller.
6. **Cache** — moka LRU keyed by `(message_hash, hint.cache_bytes(), tenant)`.

The result is fed to the LLM as a small `tools=[…]` array. Top-K defaults
to ≤ 50; the planner pre-LLM trimming keeps prompt cost bounded regardless
of how many hundreds of capabilities are registered.

### 4.4 Executor ([`executor.rs`](../../apps/backend/crates/agent-core/src/capabilities/executor.rs))

Single entry point: `ToolExecutor::invoke(registry, cap_name, tool_name, input, tenant)`.
Responsibilities:

- OpenTelemetry span `tool.cap` / `tool.name` / `tenant_id`.
- Metrics: `tool_invocations`, `tool_duration_ms`, `tool_errors`.
- Provider lookup → `provider.invoke(tool_name, input, tenant)`.
- Tool-name sanitisation (`.` → `_`) and the inverse fallback in
  `SemanticCapabilityRouter::invoke` so model-emitted names round-trip.

#### Plan execution

`run_plan(steps, registry, llm, tenant, realtime)` consumes the
`PlanStep[]` array produced by [`plan-orchestrate`](#plan-orchestrate) or
[`plan-on-upload`](#plan-on-upload) and executes each step with one of three
strategies:

| Strategy              | Behaviour                                                                                          |
| --------------------- | -------------------------------------------------------------------------------------------------- |
| `single`              | Invoke once and pass the output forward.                                                            |
| `parallel_consensus`  | Invoke the same tool twice in parallel; on equal results return either, else ask a cheap LLM to choose. Requires `idempotent = true`. |
| `fallback_cascade`    | Invoke; on error fall back (currently echoes input — future: try next-best capability).             |

Realtime events `pipeline.step.started` / `pipeline.step.finished` are
published on the tenant's broadcast channel so the web/desktop shell can render
a live pipeline timeline without polling.

### 4.5 Admin & dynamic registration ([`admin.rs`](../../apps/backend/crates/agent-core/src/capabilities/admin.rs))

`CapabilityAdmin` coordinates `RegisteredToolStore` (filesystem persistence),
`CapabilityRegistry`, `RegisteredToolValidator`, and `AuditStore` for:

- `list` / `get` / `get_manifest_toml`
- `create` (with size + WASM-size limits — `AdminLimits` defaults: 64 caps,
  64 KiB manifest, 8 MiB WASM)
- `update`, `delete`, `set_enabled`, `reload`
- `test_invoke` — executes the tool with arbitrary input for QA.

Limits are environment-tunable:
`CONUSAI_MAX_CAPABILITIES`, `CONUSAI_MAX_MANIFEST_BYTES`, `CONUSAI_MAX_WASM_BYTES`.

#### `POST /admin/capabilities/register`

Authenticated by `Bearer ${PLATFORM_ADMIN_TOKEN}` (or `X-Tenant-ID: dev`
in dev mode when `JWT_SECRET` is unset — see
[`agent-gateway/src/routes/mod.rs`](../../apps/backend/crates/agent-gateway/src/routes/mod.rs)).
Body is a `ToolManifest` plus an `endpoint` URL; the gateway:

1. Validates the manifest.
2. Inserts a row into `capability_specs` with `strategy = "remote_mcp"`.
3. Builds a `RemoteMcpCapability` and `registry.replace(provider)`s it.
4. Schedules a Qdrant re-embed so the router sees the new capability immediately.

This is how a containerised, polyglot service joins the platform without
touching Rust code — see §6.

---

## 5. On-disk capability inventory

Every directory below ships in
[`apps/backend/capabilities/`](../../apps/backend/capabilities/) and is loaded at
gateway startup. Categories follow [taxonomy.md](taxonomy.md).

### 5.1 `sense` — classify / detect

| Cap dir                                                                                          | Namespace                  | Kind  | Purpose                                                                  |
| ------------------------------------------------------------------------------------------------ | -------------------------- | ----- | ------------------------------------------------------------------------ |
| [sense-mime](../../apps/backend/capabilities/sense-mime/capability.toml)                         | `sense.detection.mime_type`| wasm  | Magic-byte MIME sniffing — offline, deterministic, sub-millisecond.       |
| [sense-classify-document](../../apps/backend/capabilities/sense-classify-document/capability.toml)| `sense.classify_document`  | chain | LLM classifier → invoice / contract / PO / receipt / report / letter / form / other (uses Haiku for cost). |

### 5.2 `extract` — pull structured data from documents

| Cap dir                                                                                                                                | Namespace                       | Kind  | Output                                                                            |
| -------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------- | ----- | --------------------------------------------------------------------------------- |
| [extract-ocr-tesseract](../../apps/backend/capabilities/extract-ocr-tesseract/capability.toml)                                         | `extract.text.ocr_tesseract`    | wasm  | Offline OCR with per-word confidence; `cost_hint = "low"`.                         |
| [extract-ocr-vision](../../apps/backend/capabilities/extract-ocr-vision/capability.toml)                                               | `extract.text.ocr_vision`       | chain | Claude vision OCR returning `{ text, confidence, language }`; `cost_hint = "high"`.|
| [ocr-service](../../apps/backend/capabilities/ocr-service/capability.toml)                                                             | `extract.ocr.vision`            | chain | Raw plain-text vision OCR (no structure) — sibling of `extract-ocr-vision`.        |
| [invoice-processing](../../apps/backend/capabilities/invoice-processing/capability.toml)                                               | `extract.fields.invoice`        | chain | Typed `InvoiceData` (+ `validate_invoice`); vision; 20 MiB PDF/image accepts.      |
| [contract-processing](../../apps/backend/capabilities/contract-processing/capability.toml)                                             | `extract.fields.contract`       | chain | `ContractData` (parties, term, clauses) + plain-language `summarise_contract`.     |
| [extract-fields-cv](../../apps/backend/capabilities/extract-fields-cv/capability.toml)                                                 | `extract.fields.cv`             | chain | `CandidateData` + `score_cv` against a job spec.                                   |
| [extract-fields-medical-claim](../../apps/backend/capabilities/extract-fields-medical-claim/capability.toml)                           | `extract.fields.medical_claim`  | chain | `MedicalClaimData` + `validate_medical_claim`.                                     |
| [extract-fields-incident](../../apps/backend/capabilities/extract-fields-incident/capability.toml)                                     | `extract.fields.incident`       | chain | `IncidentData` + `assess_severity` triage tool.                                    |

### 5.3 `convert` — change representation

| Cap dir                                                                                          | Namespace                          | Kind   | Notes                                                                  |
| ------------------------------------------------------------------------------------------------ | ---------------------------------- | ------ | ---------------------------------------------------------------------- |
| [convert-pdf-to-md](../../apps/backend/capabilities/convert-pdf-to-md/capability.toml)           | `convert.document.pdf_to_markdown` | chain  | Claude Opus PDF → Markdown; `requires = ["extract-ocr-vision"]`.        |
| [convert-audio-to-text](../../apps/backend/capabilities/convert-audio-to-text/capability.toml)   | `convert.audio_to_text`            | native | Enqueues Whisper job — returns `task_id`; poll `GET /v1/tasks/{id}`.    |
| [transcribe-video](../../apps/backend/capabilities/transcribe-video/capability.toml)             | `convert.audio_to_text`            | native | Same namespace, disabled by default — alternative job-backed wiring registered programmatically in `state.rs`. |

### 5.4 `compose` — author output for humans/systems

| Cap dir                                                                                          | Namespace             | Kind  | Notes                                                                                  |
| ------------------------------------------------------------------------------------------------ | --------------------- | ----- | -------------------------------------------------------------------------------------- |
| [compose-email](../../apps/backend/capabilities/compose-email/capability.toml)                   | `compose.email`       | chain | Subject + body + tone (`professional` / `friendly` / `formal`) from arbitrary context. |
| [compose-report-md](../../apps/backend/capabilities/compose-report-md/capability.toml)           | `compose.report_md`   | chain | Markdown report for humans.                                                            |
| [compose-report-json](../../apps/backend/capabilities/compose-report-json/capability.toml)       | `compose.report_json` | chain | Machine-bound JSON with metadata envelope (`report_id`, `created_at`, `schema_version`).|

### 5.5 `plan` — choose and sequence work

| Cap dir                                                                                          | Namespace           | Kind   | Notes                                                                                                                                                                                |
| ------------------------------------------------------------------------------------------------ | ------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| <a id="plan-orchestrate"></a>[plan-orchestrate](../../apps/backend/capabilities/plan-orchestrate/capability.toml) | `plan.orchestrate`  | chain  | Meta-capability: emits a `PlanStep[]` with one of `single` / `parallel_consensus` / `fallback_cascade`. Two tools: `orchestrate(intent, mimes, available_capabilities)` and `route_by_mime(mimes)`. |
| <a id="plan-on-upload"></a>[plan-on-upload](../../apps/backend/capabilities/plan-on-upload/capability.toml)       | `plan.on_upload`    | native | Per-tenant policy: `(object_key, filename, content_type, size) → PlanStep[]`. Default pipeline = OCR vision → classify document. Admins customise via `[[config.steps]]`.            |

### 5.6 `storage` — workspace I/O

All except `file-storage` (MCP wrapper around RustFS/S3) are `native` Rust
providers dispatched by `config.op`. Workspaces are per-tenant; see
[ops/rustfs.md](../ops/rustfs.md).

| Cap dir                                                                                              | Namespace                       | Kind   | Tool(s) → effect                                                       |
| ---------------------------------------------------------------------------------------------------- | ------------------------------- | ------ | ---------------------------------------------------------------------- |
| [file-storage](../../apps/backend/capabilities/file-storage/capability.toml)                         | `storage.object`                | mcp    | `upload_file`, `download_file`, `presigned_url` (RustFS / AWS S3).      |
| [storage-workspace](../../apps/backend/capabilities/storage-workspace/capability.toml)               | `storage.workspace`             | native | `save_document` (markdown to top-level folder), `list_folders`.         |
| [storage-workspace-move](../../apps/backend/capabilities/storage-workspace-move/capability.toml)     | `storage.node.move`             | native | `move_node` — relocate workspace node by ULID.                          |
| [storage-put](../../apps/backend/capabilities/storage-put/capability.toml)                           | `storage.put`                   | native | `put_object` — write text or base64 binary.                             |
| [storage-read-text](../../apps/backend/capabilities/storage-read-text/capability.toml)               | `storage.fs.read`               | native | `read_file` — read UTF-8 text.                                          |
| [storage-write-text](../../apps/backend/capabilities/storage-write-text/capability.toml)             | `storage.fs.write`              | native | `write_file` — UTF-8 write with auto-mkdir parents.                     |
| [storage-move](../../apps/backend/capabilities/storage-move/capability.toml)                         | `storage.object.move`           | native | `move_object` — rename/relocate a file path.                            |
| [storage-tag](../../apps/backend/capabilities/storage-tag/capability.toml)                           | `storage.object.tag`            | native | `tag_object` — write key/value `.meta.json` sidecar.                    |
| [storage-list-folders](../../apps/backend/capabilities/storage-list-folders/capability.toml)         | `storage.object.list`           | native | `list_folders` — enumerate files & dirs under prefix.                   |
| [storage-create-folder](../../apps/backend/capabilities/storage-create-folder/capability.toml)       | `storage.folder.create`         | native | `create_folder` — create a workspace folder node.                       |
| [storage-ensure-folder](../../apps/backend/capabilities/storage-ensure-folder/capability.toml)       | `storage.object.ensure_folder`  | native | `ensure_folder` — idempotent mkdir-p.                                   |
| [storage-ensure-date-folder](../../apps/backend/capabilities/storage-ensure-date-folder/capability.toml)| `storage.ensure_date_folder` | native | `ensure_date_folder` — `<root>/YYYY/MM/DD`.                             |
| [storage-find-by-name](../../apps/backend/capabilities/storage-find-by-name/capability.toml)         | `storage.node.find_by_name`     | native | `find_by_name` — exact-name lookup in tenant workspace.                 |
| [storage-show-tree](../../apps/backend/capabilities/storage-show-tree/capability.toml)               | `storage.tree.show`             | native | `show_tree` — bounded Markdown tree (depth 1–5, default 2).             |
| [storage-delete](../../apps/backend/capabilities/storage-delete/capability.toml)                     | `storage.node.delete`           | native | `delete_node` — single node.                                            |
| [storage-bulk-delete](../../apps/backend/capabilities/storage-bulk-delete/capability.toml)           | `storage.node.bulk_delete`      | native | `bulk_delete` — atomic recursive delete; requires explicit confirmation.|

### 5.7 `deliver` — push to external systems

| Cap dir                                                                                          | Namespace                  | Kind | Status                                                       |
| ------------------------------------------------------------------------------------------------ | -------------------------- | ---- | ------------------------------------------------------------ |
| [google-workspace](../../apps/backend/capabilities/google-workspace/capability.toml)             | `deliver.google.workspace` | mcp  | Drive / Docs / Sheets / Gmail; OAuth2; `enabled = false` until tenant connects an account. |

### 5.8 `compute` — engine-level utilities

| Cap dir                                                                                          | Namespace                 | Kind  | Notes                                                       |
| ------------------------------------------------------------------------------------------------ | ------------------------- | ----- | ----------------------------------------------------------- |
| [runtime-echo](../../apps/backend/capabilities/runtime-echo/capability.toml)                     | `compute.debug.echo`      | chain | Smoke-tests the chain path end-to-end.                       |
| [template-wasm](../../apps/backend/capabilities/template-wasm/capability.toml)                   | `compute.debug.wasm-ping` | wasm  | Returns `42`; minimum viable WASM template for new authors. |

### 5.9 Inter-capability requirements (selected)

```
convert-pdf-to-md ──requires──▶ extract-ocr-vision
plan-on-upload    ──steps────▶ extract-ocr-vision, sense-classify-document
plan-orchestrate  ──executes─▶ <whatever router top-K returns>
```

`requires` is advisory: load order is unchanged, but the router logs a
warning if any required capability is unregistered.

---

## 6. Out-of-process capabilities: the `current-time` reference

[`services/current-time`](../../services/current-time/) is the canonical
example of a **zero-core-touch** capability — a Docker container that
self-registers at startup, with no Rust changes required.

### 6.1 Container

[docker-compose.yml](../../docker-compose.yml) declares:

```yaml
current-time:
  build: { context: ./services/current-time }
  environment:
    GATEWAY_URL: "http://host.docker.internal:8080"
    PLATFORM_ADMIN_TOKEN: "${PLATFORM_ADMIN_TOKEN:-}"
    SERVICE_URL: "http://host.docker.internal:8082"
  ports: ["8082:8082"]
  depends_on: { agent-gateway: { condition: service_started } }
```

### 6.2 Lifecycle

1. **Boot.** FastAPI starts on `:8082`; `@app.on_event("startup")` schedules
   `register_with_retry()` as a background task — the HTTP server is up before
   registration completes so health checks succeed regardless.
2. **Registration.** Up to 25 attempts (5 s apart) POST a `MANIFEST` object to
   `${GATEWAY_URL}/admin/capabilities/register` with
   `Authorization: Bearer ${PLATFORM_ADMIN_TOKEN}`. In dev (no JWT secret) the
   service falls back to `X-Tenant-ID: dev`.
3. **Manifest.** The payload declares
   `kind = "remote_mcp"`, `namespace = "media.time.current-time"`, and the MCP
   endpoint `${SERVICE_URL}/mcp`. The gateway stores it in `capability_specs`
   and constructs a `RemoteMcpCapability` that dispatches to the endpoint.
4. **Invocation.** When the LLM emits `media_time_current-time__get_current_time`,
   `ToolExecutor::invoke` resolves the provider, which calls `McpAdapter::call_tool`,
   which POSTs JSON-RPC 2.0 `tools/call` to `http://current-time:8082/mcp`.
5. **Tool.** `get_current_time(timezone?)` returns
   `{ content, artifacts: [], metadata: { timezone, timestamp } }`.

### 6.3 Wire protocol

The service speaks MCP JSON-RPC 2.0 over plain HTTP POST — sufficient for the
adapter and free of WebSocket/stdio complexity:

```http
POST /mcp
{ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }

POST /mcp
{ "jsonrpc": "2.0", "id": 2, "method": "tools/call",
  "params": { "name": "get_current_time", "arguments": { "timezone": "Europe/Helsinki" } } }
```

Unknown methods return `-32601`; parse errors return `-32700`.

### 6.4 Why this matters

- Any language, any process, any host can join the platform — Rust, Python,
  Go, Node. The only contract is `POST /mcp` + JSON-RPC.
- No agent-gateway rebuild, deploy, or coordination required.
- Manifest changes are picked up by re-registering — semantic router
  re-embeds on `replace()`.

---

## 7. Operational characteristics

### 7.1 Boot sequence

1. Gateway starts → `LlmRegistry` built → `CapabilityRegistry::with_default_factories(llm)` registers `Mcp` / `Wasm` / `Chain` factories.
2. `CapabilityDiscovery::from_env().discover_into(&mut registry)` loads every TOML from `CONUSAI_CAPABILITIES_DIR`.
3. `registry.run_bulk_load()` pulls DB-backed capabilities (`remote_mcp` rows + `dynamic_prompt` rows).
4. Programmatic registrations (e.g. `transcribe-video` with `JobExecutor`) happen in `state.rs`.
5. Embeddings recomputed; Qdrant collection upserted; `ManifestWatcher` started.

### 7.2 Failure modes

| Symptom                                  | Likely cause                                                                | Where to look                                                                 |
| ---------------------------------------- | --------------------------------------------------------------------------- | ----------------------------------------------------------------------------- |
| Card present, `last_error` set           | Factory `create()` failed — bad WASM, missing `[chain]`, invalid TOML        | `admin.rs` audit; `card.last_error`                                          |
| LLM never picks an obviously-relevant cap| Embedding text too sparse; missing `search_keywords`                         | `manifest.embedding_text()`; expand `search_keywords` / `tags` and reload    |
| Cap routed but invoke fails 404          | Tool name not on the provider                                               | Check `tools[]` block and Anthropic-safe name sanitisation                    |
| Remote MCP unreachable                   | Service down or registration retries exhausted                              | `current-time` logs; `/admin/capabilities` shows `last_error`                 |
| Hot-reload didn't pick up edit           | Watcher not running (mount missing) or wrong dir                            | Confirm `./apps/backend/capabilities` is bind-mounted                         |

### 7.3 Limits & quotas (defaults; env-tunable)

- `CONUSAI_MAX_CAPABILITIES = 64`
- `CONUSAI_MAX_MANIFEST_BYTES = 65_536`
- `CONUSAI_MAX_WASM_BYTES = 8_388_608`
- Router `top_k ≤ 50` per turn
- Chain `max_tokens` defaults to 2048; the reducer used by `parallel_consensus` caps at 2048.

### 7.4 Observability

Each invocation emits:

- OTel span `tool.cap`, `tool.name`, `tenant_id`, `error.type`.
- Metrics `tool_invocations`, `tool_duration_ms`, `tool_errors` labelled `{capability, tool}`.
- Realtime events on the tenant channel: `pipeline.step.started`, `pipeline.step.finished`.
- System events: `capability.reloaded` on `__system__`.

Pipe to Jaeger via the optional `observability` profile in
[docker-compose.yml](../../docker-compose.yml).

---

## 8. Authoring checklist

When adding a capability — minimal happy path:

1. `mkdir apps/backend/capabilities/<my-cap>/`
2. Write `capability.toml` with `schema_version = "2.0"`, `name`, `namespace`,
   `category` (taxonomy.md), `kind`, `description`, `[[tools]]`, `[tools.input_schema]`.
3. If `kind = "chain"` — add the `[chain]` block with `model`, `prompt_template`,
   and optionally `output_schema`. No Rust changes required.
4. If `kind = "wasm"` — drop the `.wasm` next to the TOML and reference it via
   `config.wasm_module`.
5. If `kind = "mcp"` — point `config.endpoint` at the MCP server.
6. If out-of-process — implement `POST /mcp` (see `current-time`) and
   POST `kind = "remote_mcp"` to `/admin/capabilities/register` at startup.
7. Add `search_keywords` aggressively — this is the difference between the
   router finding your capability and never picking it.
8. Save → `ManifestWatcher` reloads in-process; verify with
   `GET /v1/capabilities` and a `test_invoke` admin call.

For a deeper walkthrough see
[how-to-add-a-domain.md](how-to-add-a-domain.md) and the
[capability authoring guide](../capability-authoring-guide.md).
