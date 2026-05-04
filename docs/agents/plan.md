# Dynamic Tool Registration via `/super-admin` UI — Implementation Plan (v3)

> Source rationale: see [tools-plan.md](tools-plan.md). This document is the concrete, file-level execution plan that turns Option A (a generic LLM tool) plus a new admin UI into shippable phases.
>
> **v3 changelog (2026-05-04):** Aligned naming with the Rig 0.9+ community vocabulary (`RegisteredTool*` family, `LlmChainTool`). Added `ToolRegistry::as_tool_set()` so dynamic capabilities are consumable by pure Rig agents in one line. UI gains optional HTMX progressive enhancement (CDN, zero new deps). Validator returns a `RegisteredToolValidationError` enum instead of string soup. `RegisteredToolCard` now caches `Arc<dyn ToolProvider>` for cheap reads. Effort estimates rewritten as AI-accelerated hours + token deltas.
>
> **v2 changelog (2026-05-04):** Renamed types for Rig alignment, extracted `PromptTemplate` for reuse, introduced `CapabilityStore` trait for testability, added `CapabilityValidator`, optional Rig `Tool` impl on the generic provider.

---

## Goal

Allow a privileged `super-admin` user to register, edit, enable/disable, and remove tool capabilities **at runtime** through a web UI at `GET /super-admin`, without restarting the gateway or shipping new Rust code for the common cases (LLM chains, MCP endpoints, WASM modules).

## Non-goals

- Multi-tenant tool isolation (all tools are platform-global today; per-tenant whitelisting is a follow-up).
- Editing built-in (`native`) tools or hardcoded chains (`invoice-processing`, `contract-processing`, `ocr-service`) — those remain code-only until explicitly migrated in Phase 7.
- Marketplace / signed-publisher trust model.
- Premature factory abstraction. The current `if chain.is_some() { ... } else { match name }` shape is the correct transitional design — no `HashMap<String, Box<dyn ProviderFactory>>` registry is needed.
- `parking_lot::RwLock` swap (revisit only if Mutex contention is measured).

---

## Naming Decisions (v3 — Rig-aligned)

| Concept | Type name | Rationale |
|---|---|---|
| Data-driven LLM tool implementation | `LlmChainTool` | Reads as "LLM chain that satisfies the `Tool` trait" — matches Rig's `Tool` vocabulary directly |
| LLM chain config in manifest | `LlmChainConfig` | Explicit; future-proofs if non-LLM chain kinds appear |
| In-process card (manifest + runtime state + cached provider) | `RegisteredToolCard` (was `ToolCard`) | Mirrors Rig's `Tool` plus admin metadata; distinguishes from on-disk `ToolManifest` |
| Admin orchestration service | `RegisteredToolAdmin` | Domain-first, shorter than `*AdminService` |
| Filesystem persistence trait | `RegisteredToolStore` | Lets us swap an in-memory store in tests, future Postgres impl |
| Validation helper | `RegisteredToolValidator` | Single home for slug + manifest + JSON Schema + kind-specific checks |
| Validation error enum | `RegisteredToolValidationError` (`thiserror`) | Typed errors, no stringly-typed soup |
| Prompt interpolation | `PromptTemplate` | Reusable across tools, evals, audit |

The existing `ToolProvider` trait, `ToolRegistry`, `ToolManifest`, `ToolKind`, and `ToolDef` keep their names — they are the public contract.

---

## Current Codebase Anchors

| Concern | Location |
|---|---|
| Tool manifest struct | [crates/agent-core/src/tools/manifest.rs](apps/backend/crates/agent-core/src/tools/manifest.rs) |
| Card struct (rename target) | [crates/agent-core/src/tools/card.rs](apps/backend/crates/agent-core/src/tools/card.rs) |
| Registry (in-memory) | [crates/agent-core/src/tools/registry.rs](apps/backend/crates/agent-core/src/tools/registry.rs) |
| Discovery (filesystem) | [crates/agent-core/src/tools/discovery.rs](apps/backend/crates/agent-core/src/tools/discovery.rs) |
| Chain factory (hardcoded match) | [crates/agent-core/src/tools/providers/chain.rs](apps/backend/crates/agent-core/src/tools/providers/chain.rs) |
| MCP factory | [crates/agent-core/src/tools/providers/mcp.rs](apps/backend/crates/agent-core/src/tools/providers/mcp.rs) |
| WASM factory + loader | [crates/agent-core/src/tools/providers/wasm.rs](apps/backend/crates/agent-core/src/tools/providers/wasm.rs) |
| `AppState` (registry mutex) | [crates/agent-gateway/src/state.rs](apps/backend/crates/agent-gateway/src/state.rs) |
| API routes assembly | [crates/agent-gateway/src/routes/mod.rs](apps/backend/crates/agent-gateway/src/routes/mod.rs) |
| `/v1/capabilities` (read-only listing) | [crates/agent-gateway/src/routes/capabilities.rs](apps/backend/crates/agent-gateway/src/routes/capabilities.rs) |
| UI router | [crates/agent-gateway/src/ui/routes.rs](apps/backend/crates/agent-gateway/src/ui/routes.rs) |
| UI handlers | [crates/agent-gateway/src/ui/handlers/](apps/backend/crates/agent-gateway/src/ui/handlers/) |
| Templates (Askama) | [crates/agent-gateway/templates/](apps/backend/crates/agent-gateway/templates/) |
| Session / cookie auth | [crates/agent-gateway/src/ui/session.rs](apps/backend/crates/agent-gateway/src/ui/session.rs) |
| JWT claims | [crates/agent-core/src/context/tenant.rs](apps/backend/crates/agent-core/src/context/tenant.rs) |
| Capability TOMLs | [apps/backend/capabilities/](apps/backend/capabilities/) |

### Key facts that constrain the design

1. `ToolRegistry` already lives behind `Mutex<ToolRegistry>` in `AppState` — runtime mutability exists; only the *factories* and *chain match* are compile-time.
2. Capabilities load at startup from `CONUSAI_CAPABILITIES_DIR` (default `./capabilities`). The registry currently has **no remove method** and **no enabled/disabled state**.
3. `TenantClaims` has no `role` field; `SessionUser` carries only `name` + `plan`. Super-admin does not exist yet.
4. UI is server-rendered Askama; no separate frontend bundler.
5. WASM and MCP capabilities are already fully data-driven; chains are hardcoded.
6. `std::sync::Mutex` is sufficient — admin writes are rare, reads clone `Arc<dyn ToolProvider>` cheaply.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│  /super-admin UI (Askama + optional HTMX CDN, server-rendered)          │
│   ─ List registered tools (status, kind, version, tools, last error)    │
│   ─ Create form: kind picker + manifest editor + (wasm upload | url)    │
│   ─ Edit / Enable / Disable / Delete                                    │
│   ─ "Reload from disk" + "Test invoke" panel (hx-post for live UX)      │
└──────────────────────┬──────────────────────────────────────────────────┘
                       │ POST /admin/capabilities, etc. (cookie-auth, role-gated)
                       ▼
┌─────────────────────────────────────────────────────────────────────────┐
│  RegisteredToolAdmin                                                    │
│   ├─ RegisteredToolValidator  (slug + manifest + JSON Schema + kind)    │
│   ├─ RegisteredToolStore      (filesystem impl; trait for testability)  │
│   └─ ToolRegistry mutation    (register / unregister / replace)         │
│   Emits audit log entry per change.                                     │
└──────────────────────┬──────────────────────────────────────────────────┘
                       │
        ┌──────────────┴──────────────┐
        ▼                             ▼
┌─────────────────────┐    ┌─────────────────────────────────────┐
│ FilesystemStore     │    │ Live ToolRegistry (Mutex<...>)      │
│  capabilities/<n>/  │    │  ─ register / unregister / replace  │
│   capability.toml   │    │  ─ enabled / disabled flag          │
│   capability.wasm   │    │  ─ last error / last reload time    │
│   state.json        │    │  ─ as_tool_set() → rig::ToolSet     │
└─────────────────────┘    └─────────────────────────────────────┘
```

### State persistence model

- **Source of truth on disk:** `capabilities/<name>/capability.toml` plus a sibling `state.json` for `{ enabled, created_at, updated_at }`.
- **Source of truth in process:** the `ToolRegistry` Mutex.
- A `reload` operation re-reads the directory and reconciles with the live registry.
- WASM bytes live as `capability.wasm` next to the manifest (existing layout).
- MCP capabilities are pure manifest — no extra files.

This keeps disk + memory coherent across restarts and lets ops still hand-edit TOMLs.

---

## Phase 0 — Prerequisites (auth + role + audit)

### 0.1 Add `role` to session and JWT claims

**Files:**
- `crates/agent-core/src/context/tenant.rs` — add `enum UserRole { User, Admin, SuperAdmin }`; add `pub role: UserRole` to `TenantContext`; add `pub role: UserRole` to `TenantClaims` with `#[serde(default)]`.
- `crates/agent-gateway/src/ui/session.rs` — add `role: String` to `SessionUser`; update `sign()` / `verify()` payload.
- `crates/agent-gateway/src/routes/auth.rs` — accept role lookup via env var `SUPER_ADMIN_EMAILS` (comma-separated); set `role` in `TenantClaims`.
- `crates/agent-gateway/src/ui/handlers/auth.rs` — set role on session cookie at login.

**Acceptance:** `cargo test --workspace` green; a logged-in user whose email is in `SUPER_ADMIN_EMAILS` carries `role = "super_admin"` in both UI session and JWT.

### 0.2 Admin role-gating middleware

**New file:** `crates/agent-gateway/src/mw/admin.rs`

```rust
pub async fn require_super_admin(
    user: SessionUser,           // FromRequestParts → 401 if no session
    req: Request, next: Next,
) -> Result<Response, StatusCode> {
    if user.role != "super_admin" { return Err(StatusCode::FORBIDDEN); }
    Ok(next.run(req).await)
}
```

For JWT-authed `/admin/*` REST routes, equivalent middleware reading `TenantClaims.role`.

### 0.3 Audit hook

`AuditStore` already exists in `AppState`. Define audit entry kinds: `tool_created`, `tool_updated`, `tool_deleted`, `tool_enabled`, `tool_disabled`, `tool_reloaded`. Emit one per mutation from `RegisteredToolAdmin`.

**Effort:** ~1 h (AI). **Tokens:** ~450.

---

## Phase 1 — `LlmChainTool` + `PromptTemplate`

> Eliminates the hardcoded match in `ChainFactory` for any new chain. Existing typed providers stay until Phase 7.

### 1.1 Reusable `PromptTemplate` (extracted)

**New file:** `crates/agent-core/src/prompt/template.rs` (~40 LOC, no new deps)

```rust
#[derive(Debug, Clone)]
pub struct PromptTemplate { template: String }

impl PromptTemplate {
    pub fn new(template: impl Into<String>) -> Self { … }

    /// Renders {{input.field}}, {{tenant.id}}, {{tenant.plan}} etc.
    /// Walks JSON paths with dot-separated keys.
    pub fn render(&self, ctx: &serde_json::Value) -> common::error::Result<String> { … }
}
```

**Rationale:** used by `LlmChainTool` today, by eval prompts and audit messages later. Pure function — trivial unit tests with `rstest` / `insta`. SRP win at near-zero cost.

### 1.2 Extend `ToolManifest`

**File:** `crates/agent-core/src/tools/manifest.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmChainConfig {
    pub model: String,
    #[serde(default)]
    pub system_prompt: Option<String>,
    pub prompt_template: String,
    #[serde(default)]
    pub vision: bool,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default)]
    pub output_schema: Option<serde_json::Value>,  // JSON Schema
}

fn default_max_tokens() -> u32 { 2048 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolManifest {
    // existing fields …
    #[serde(default)]
    pub chain: Option<LlmChainConfig>,
}
```

### 1.3 Implement `LlmChainTool`

**New file:** `crates/agent-core/src/chains/llm_chain.rs`

- Struct `LlmChainTool { manifest, chain_cfg, prompt: PromptTemplate, schema: Option<JsonSchema> }`.
- `invoke(tool_name, input, tenant)`:
  1. Build context `Value` from `{ input: <input>, tenant: <tenant view> }`.
  2. Render `prompt_template` via `PromptTemplate::render`.
  3. If `vision`, pull `image_path` from input (reuse `resolve_image_path`).
  4. Call `LlmRegistry::get(model).complete(...)` (existing infra).
  5. Parse Claude response; if `output_schema`, validate via `jsonschema` crate.
  6. Return `Value`.

### 1.4 Optional — implement Rig's `Tool` trait

When `rig-core` is available, expose `LlmChainTool` as a `rig::tool::Tool` (one trait impl block, ~25 LOC). This lets pure-Rig agents consume dynamic capabilities directly via `ToolSet`. Behind a `rig` feature flag.

```rust
#[cfg(feature = "rig")]
impl rig::tool::Tool for LlmChainTool { /* delegates to invoke() */ }
```

### 1.5 Update `ChainFactory`

**File:** `crates/agent-core/src/tools/providers/chain.rs`

```rust
fn create(&self, card: RegisteredToolCard) -> anyhow::Result<Arc<dyn ToolProvider>> {
    if card.manifest.chain.is_some() {
        return Ok(Arc::new(LlmChainTool::new(card)?));
    }
    match card.manifest.name.as_str() {
        "invoice-processing"  => Ok(Arc::new(InvoiceProvider::new(card))),
        "contract-processing" => Ok(Arc::new(ContractProvider::new(card))),
        "ocr-service"         => Ok(Arc::new(OcrProvider::new(card))),
        other => anyhow::bail!("unknown chain tool: {other} (and no [chain] in manifest)"),
    }
}
```

This is the correct transitional shape — **do not** introduce a generic `HashMap<String, Box<dyn ProviderFactory>>` registry yet.

### 1.6 Tests

- Unit: `PromptTemplate::render` with nested paths, missing keys, escaping.
- Unit: parse a manifest with `[chain]`, schema validation pass + fail.
- Integration: drop a `capabilities/test-extractor/capability.toml` with `[chain]`, restart in test, confirm it appears in `/v1/capabilities` and invokes correctly.
- Rig: with `--features rig`, build a `ToolSet` from the registry and invoke the dynamic tool through it.

### 1.7 Acceptance

- `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` clean.
- New TOML-only capability works end-to-end without any Rust changes.

**Effort:** ~2.5 h (AI). **Tokens:** ~1 800.

---

## Phase 2 — Registry mutability + Rig export

### 2.1 Rename `ToolCard` → `RegisteredToolCard`

Mechanical rename via `vscode_renameSymbol` across the workspace.

### 2.2 Add lifecycle methods + Rig export to `ToolRegistry`

**File:** `crates/agent-core/src/tools/registry.rs`

```rust
impl ToolRegistry {
    pub fn unregister(&mut self, name: &str) -> bool { … }
    pub fn replace(&mut self, provider: Arc<dyn ToolProvider>) { … }
    pub fn set_enabled(&mut self, name: &str, enabled: bool) -> bool { … }
    pub fn reload_capability(&mut self, dir: &Path) -> Result<()> { … }

    /// Export currently-enabled providers as a Rig ToolSet (~15 LOC).
    #[cfg(feature = "rig")]
    pub fn as_tool_set(&self) -> rig::ToolSet { … }
}
```

### 2.3 Per-card runtime state on `RegisteredToolCard`

```rust
pub struct RegisteredToolCard {
    pub manifest: ToolManifest,
    pub source_dir: PathBuf,
    pub enabled: bool,
    pub last_error: Option<String>,
    pub registered_at: SystemTime,
    pub updated_at: SystemTime,
    pub provider: Arc<dyn ToolProvider>,   // cached, cheap to clone
}
```

Agent execution paths skip `enabled = false`; admin listings include them.

### 2.4 Filter capabilities used by agents

Update `executor.rs` and `/v1/capabilities` to honour `enabled`.

### 2.5 Tests

- Register / unregister / replace round-trip.
- Disabled tool absent from `/v1/capabilities` but present in `/admin/capabilities`.
- `as_tool_set()` returns only enabled providers.

**Effort:** ~1.5 h (AI). **Tokens:** ~900.

---

## Phase 3 — `RegisteredToolAdmin` + `RegisteredToolStore` + `RegisteredToolValidator`

> Pure logic layer. No HTTP. Sits between admin routes and the registry/filesystem.

### 3.1 `RegisteredToolStore` trait (testability)

**New file:** `crates/agent-core/src/tools/store.rs`

```rust
pub trait RegisteredToolStore: Send + Sync {
    fn list(&self) -> Result<Vec<String>>;
    fn read_manifest(&self, name: &str) -> Result<String>;
    fn write_manifest(&self, name: &str, toml: &str) -> Result<()>;
    fn write_wasm(&self, name: &str, bytes: &[u8]) -> Result<()>;
    fn read_state(&self, name: &str) -> Result<Option<RegisteredToolState>>;
    fn write_state(&self, name: &str, state: &RegisteredToolState) -> Result<()>;
    fn delete(&self, name: &str) -> Result<()>;
    fn capability_dir(&self, name: &str) -> PathBuf;
}

pub struct FilesystemStore { root: PathBuf }
impl RegisteredToolStore for FilesystemStore { … }

#[cfg(test)]
pub struct InMemoryStore { … }
```

Atomic semantics in `FilesystemStore`: write into a `<name>.tmp` directory then `rename` to `<name>`.

### 3.2 `RegisteredToolValidator`

**New file:** `crates/agent-core/src/tools/validator.rs`

```rust
#[derive(Debug, thiserror::Error)]
pub enum RegisteredToolValidationError {
    #[error("invalid name: {0}")]                InvalidName(String),
    #[error("manifest parse error: {0}")]        ManifestParse(String),
    #[error("invalid JSON schema in {field}: {message}")]
    InvalidSchema { field: String, message: String },
    #[error("MCP endpoint disallowed: {0}")]     McpEndpointDisallowed(String),
    #[error("WASM module rejected: {0}")]        WasmRejected(String),
    #[error("size limit exceeded: {what} = {actual} > {limit}")]
    SizeLimit { what: &'static str, actual: usize, limit: usize },
}

pub struct ValidationReport {
    pub errors: Vec<RegisteredToolValidationError>,
    pub warnings: Vec<String>,
}
impl ValidationReport { pub fn ok(&self) -> bool { self.errors.is_empty() } }

pub struct RegisteredToolValidator;
impl RegisteredToolValidator {
    pub fn validate_manifest(toml: &str) -> ValidationReport { … }
    pub fn validate_kind_specific(manifest: &ToolManifest, payload: &KindPayload) -> ValidationReport { … }
    pub fn validate_name(name: &str) -> ValidationReport { … }   // slug regex ^[a-z0-9-]{2,64}$
}
```

Single home for slug regex, TOML parse, JSON Schema check on `output_schema`, MCP endpoint shape, WASM magic bytes.

### 3.3 `RegisteredToolAdmin`

**New file:** `crates/agent-core/src/tools/admin.rs`

```rust
pub struct RegisteredToolAdmin {
    store: Arc<dyn RegisteredToolStore>,
    registry: Arc<Mutex<ToolRegistry>>,
    audit: Arc<dyn AuditStore>,
    limits: AdminLimits,
}

impl RegisteredToolAdmin {
    pub fn list(&self) -> Vec<AdminToolView> { … }
    pub fn get(&self, name: &str) -> Option<AdminToolView> { … }
    pub fn create(&self, req: CreateRequest, actor: &str) -> Result<AdminToolView> { … }
    pub fn update(&self, name: &str, req: UpdateRequest, actor: &str) -> Result<…> { … }
    pub fn set_enabled(&self, name: &str, enabled: bool, actor: &str) -> Result<…> { … }
    pub fn delete(&self, name: &str, actor: &str) -> Result<()> { … }
    pub fn reload(&self, name: &str, actor: &str) -> Result<…> { … }
    pub fn reload_all(&self, actor: &str) -> Result<usize> { … }
    pub fn validate(&self, manifest_toml: &str) -> ValidationReport { … }
    pub fn test_invoke(
        &self, name: &str, tool: &str, input: Value, tenant: &TenantContext,
    ) -> Result<Value> { … }
}
```

### 3.4 `CreateRequest` shape

```rust
pub struct CreateRequest {
    pub manifest_toml: String,
    pub kind_payload: KindPayload,   // Mcp{}, Wasm{ bytes }, Chain{}, Native{}
}
```

### 3.5 Persistence flow (create)

1. `RegisteredToolValidator::validate_name` → `validate_manifest` → `validate_kind_specific`.
2. Check uniqueness against `store.list()` and the live registry.
3. `store.write_manifest()` (+ `write_wasm` if applicable) atomically.
4. Build `RegisteredToolCard`, dispatch to factory, register in registry under Mutex.
5. `store.write_state()` with `enabled = true`, timestamps.
6. Emit audit entry.

### 3.6 Atomicity & rollback

- `FilesystemStore` writes via `<name>.tmp/` → `rename`.
- If registry insert fails, `store.delete(name)` and bubble the error.

### 3.7 Concurrency

Single Mutex on the registry serialises admin writes. Reads bypass via `Arc<dyn ToolProvider>` clones cached on `RegisteredToolCard`.

### 3.8 Tests

- All service methods tested with `InMemoryStore` — fast, no disk I/O.
- One integration test exercises `FilesystemStore` round-trip.
- Concurrent create races resolved by file-existence check.

**Effort:** ~2 h (AI). **Tokens:** ~2 200.

---

## Phase 4 — Admin REST API

**New file:** `crates/agent-gateway/src/routes/admin.rs`

| Method | Path | Purpose |
|---|---|---|
| `GET`    | `/admin/capabilities`                | List all (includes disabled, last_error) |
| `GET`    | `/admin/capabilities/{name}`         | Detail incl. raw TOML |
| `POST`   | `/admin/capabilities`                | Create (multipart for WASM) |
| `PATCH`  | `/admin/capabilities/{name}`         | Update manifest body |
| `POST`   | `/admin/capabilities/{name}/enable`  | Enable |
| `POST`   | `/admin/capabilities/{name}/disable` | Disable |
| `DELETE` | `/admin/capabilities/{name}`         | Delete |
| `POST`   | `/admin/capabilities/{name}/reload`  | Re-read TOML from disk |
| `POST`   | `/admin/capabilities/reload-all`     | Re-scan dir |
| `POST`   | `/admin/capabilities/{name}/test`    | Invoke a tool with a sample payload |
| `POST`   | `/admin/capabilities/validate`       | Dry-run validate manifest body |

Route paths stay `/admin/capabilities/*` to keep OpenAPI clean and to avoid churn with existing `/v1/capabilities`.

All routes:
- Behind `require_super_admin` middleware (JWT claim variant).
- Tagged `admin` in OpenAPI; included in `ApiDoc`.
- Request/response types in `routes/admin.rs` with `utoipa::ToSchema`.

**Wire-up:** `routes/mod.rs` exposes `admin_router()` merged into `protected_router()` with the role middleware applied.

### Tests

- Each endpoint: happy path + 403 (non-admin) + 404 + 409 (duplicate name).
- Multipart upload test for WASM.

**Effort:** ~1.5 h (AI). **Tokens:** ~1 100.

---

## Phase 5 — `/super-admin` UI (Askama + optional HTMX)

### 5.1 Routes

**File:** `crates/agent-gateway/src/ui/routes.rs`

```rust
.route("/super-admin", get(admin::index))
.route("/super-admin/capabilities/new", get(admin::new_form).post(admin::create))
.route("/super-admin/capabilities/{name}", get(admin::detail).post(admin::update))
.route("/super-admin/capabilities/{name}/delete", post(admin::delete))
.route("/super-admin/capabilities/{name}/toggle", post(admin::toggle))
.route("/super-admin/capabilities/{name}/test", post(admin::test_invoke))
.route("/super-admin/capabilities/reload-all", post(admin::reload_all))
```

All wrapped with `route_layer(from_fn(require_super_admin))`.

### 5.2 Handlers

**New file:** `crates/agent-gateway/src/ui/handlers/admin.rs`

Each handler either renders an Askama view or performs a mutation via `RegisteredToolAdmin` and redirects with a flash message (short-lived signed cookie). HTMX-targeted handlers detect the `HX-Request` header and return Askama partial fragments instead of redirects.

### 5.3 Templates

**New files** under `crates/agent-gateway/templates/super_admin/`:

| Template | Purpose |
|---|---|
| `layout.html`              | Shared chrome (extracted to keep child templates lean); single `<script src="https://unpkg.com/htmx.org@1">` |
| `list.html`                | Capabilities table (name, kind, status, tools, last_error, actions) |
| `new.html`                 | Form: kind dropdown → conditional fields (TOML editor + optional file input) |
| `detail.html`              | Editable TOML, enable/disable toggle, delete confirm, test panel |
| `partials/_test_panel.html`| Tool select + JSON input + result block (`hx-post` target) |
| `partials/_validation.html`| Inline validation report (`hx-post` target on textarea `oninput`) |
| `partials/_flash.html`     | Flash message banner |

**View structs** in `ui/view.rs`:

```rust
#[derive(Template)]
#[template(path = "super_admin/list.html")]
pub struct AdminListView { pub tools: Vec<AdminToolRow>, pub flash: Option<Flash>, … }

pub struct AdminToolRow {
    pub name: String, pub kind: String, pub version: String,
    pub enabled: bool, pub tool_count: usize,
    pub last_error: Option<String>, pub updated_at: String,
}
```

### 5.4 UX requirements

- **Editor:** plain `<textarea>` with monospace font. CodeMirror via CDN as a follow-up.
- **HTMX progressive enhancement:** one `<script>` tag, no build step. `hx-post` + `hx-target` drive:
  - Live debounced manifest validation (textarea `oninput` → `_validation.html` fragment).
  - Test invoke panel (submit JSON → `_test_panel.html` fragment with response/error).
  - Enable/disable toggle (no full-page reload).
  Forms still work without JS — handlers detect `HX-Request` and return either a redirect or a fragment.
- **WASM upload:** `<input type="file" accept=".wasm">` + multipart submit.
- **Empty / loading / error states** per the [verify skill](.claude/skills/plan-browser-verifier/SKILL.md) UI checklist.
- **Confirmation modal** for delete.
- **Flash messages** for mutations.

### 5.5 Sidebar entry

`templates/app.html` shows a "Super Admin" link in the sidebar **only when** `user_role == "super_admin"`. Pass `user_role` through `AppView`.

### 5.6 Browser verification

Use the `plan-browser-verifier` skill after each UI screen. Capture screenshots, check WCAG AA contrast, keyboard nav, loading/error/empty states for list/new/detail. Verify HTMX-driven interactions degrade gracefully when JS is disabled.

**Effort:** ~3 h (AI). **Tokens:** ~2 400.

---

## Phase 6 — Hot-reload safety & limits

### 6.1 Capability quotas

```rust
pub struct AdminLimits {
    pub max_capabilities: usize,           // default 64
    pub max_manifest_bytes: usize,         // default 64 KiB
    pub max_wasm_bytes: usize,             // default 8 MiB
    pub max_tools_per_tool_set: usize,     // default 256 (future-proofs Rig export)
    pub allowed_mcp_hosts: Vec<String>,    // empty = allow all
}
```

Enforced in `RegisteredToolAdmin::create/update`.

### 6.2 MCP endpoint allowlist

Optional regex/host-suffix allowlist. Off by default in dev, mandatory in prod (env-driven). Blocks SSRF / private-network exfiltration.

### 6.3 WASM module gating

- Reject if magic bytes `\0asm` missing.
- Reject if module exceeds size limit.
- Wasmtime fuel/memory limits already enforced by `WasmToolLoader`.

### 6.4 Audit filter

Add `GET /admin/audit?type=tool_*` filter on the existing audit endpoint.

**Effort:** ~0.5 h (AI). **Tokens:** ~300.

---

## Phase 7 — Migration of existing chain capabilities (optional)

After Phase 1 ships and the UI proves the generic path works:

1. **`ocr-service`** — single tool, simplest. Convert to `[chain]`. Delete `OcrProvider` + match arm.
2. **`invoice-processing`** — `extract_invoice` (LLM) → `[chain]`; `validate_invoice` (pure Rust) stays as a small typed provider OR moves to a `[builtin]` helper.
3. **`contract-processing`** — same shape as invoice.

After all three migrate, `ChainFactory::create()` collapses to:

```rust
fn create(&self, card: RegisteredToolCard) -> anyhow::Result<Arc<dyn ToolProvider>> {
    Ok(Arc::new(LlmChainTool::new(card)?))
}
```

…and any new chain becomes a TOML-only addition through `/super-admin`.

**Effort:** ~1 h per capability (AI). **Tokens:** ~2 500 total.

---

## Phase Sequencing & Dependencies

```
Phase 0 (auth/role)  ──┐
                       │
Phase 1 (LlmChainTool ─┤
   + PromptTemplate)   │
                       ├─→ Phase 4 (admin REST) ──→ Phase 5 (UI) ──→ Phase 6 (limits)
Phase 2 (mutability  ──┤        ▲
   + as_tool_set)      │        │
                       │        │
Phase 3 (admin +     ──┘        │
   store + validator)            │
                                 │
Phase 7 (migration) ─────────────┘   (runs after Phase 5 proves the generic path)
```

Phases 0 and 1 run in parallel. Phase 2 is independent. Phase 3 needs 2. Phases 4 and 5 can overlap once 3 lands. Phase 6 is hardening before any prod deploy.

---

## Acceptance Criteria (end-to-end)

A super-admin can:

1. Log in to `/login` with an email listed in `SUPER_ADMIN_EMAILS`.
2. See "Super Admin" link in the sidebar.
3. Open `/super-admin` and see the live list of capabilities (6 today: `google-workspace`, `ocr-service`, `contract-processing`, `file-storage`, `wasm-ping`, `invoice-processing`).
4. Click "New" and create:
   - **An MCP capability** by submitting TOML with `kind = "mcp"` and `[config] endpoint = "..."`.
   - **A WASM capability** by uploading a `.wasm` file + TOML.
   - **A chain capability** by submitting TOML with `[chain] prompt_template = "..."`.
5. The new capability appears in `/v1/capabilities` immediately, in any Rig `ToolSet` produced by `ToolRegistry::as_tool_set()`, and the agent loop can call it.
6. Editing the TOML and saving updates the live registry without a restart (live-validated via HTMX).
7. Disable hides the tool from `/v1/capabilities`; re-enable restores it.
8. Delete removes both the registry entry and the on-disk folder.
9. After a process restart, all created capabilities reload from disk in the same state (enabled/disabled preserved via `state.json`).
10. A non-admin user gets `403` on every `/admin/*` and `/super-admin/*` route.
11. `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` clean.
12. Browser verification (per the `plan-browser-verifier` skill) passes contrast + keyboard + empty/loading/error checks for list, new, and detail screens, with and without JS.

---

## File Change Summary

### New files

- `crates/agent-core/src/prompt/template.rs`           — `PromptTemplate`
- `crates/agent-core/src/chains/llm_chain.rs`          — `LlmChainTool` (+ optional `rig::Tool` impl)
- `crates/agent-core/src/tools/store.rs`               — `RegisteredToolStore` trait + `FilesystemStore` + `InMemoryStore` (test)
- `crates/agent-core/src/tools/validator.rs`           — `RegisteredToolValidator` + `RegisteredToolValidationError`
- `crates/agent-core/src/tools/admin.rs`               — `RegisteredToolAdmin`
- `crates/agent-gateway/src/mw/admin.rs`               — `require_super_admin`
- `crates/agent-gateway/src/routes/admin.rs`           — admin REST endpoints
- `crates/agent-gateway/src/ui/handlers/admin.rs`      — UI handlers
- `crates/agent-gateway/templates/super_admin/layout.html`
- `crates/agent-gateway/templates/super_admin/list.html`
- `crates/agent-gateway/templates/super_admin/new.html`
- `crates/agent-gateway/templates/super_admin/detail.html`
- `crates/agent-gateway/templates/super_admin/partials/_test_panel.html`
- `crates/agent-gateway/templates/super_admin/partials/_validation.html`
- `crates/agent-gateway/templates/super_admin/partials/_flash.html`

### Modified files

- `crates/agent-core/src/tools/manifest.rs` — `LlmChainConfig` + `chain` field
- `crates/agent-core/src/tools/card.rs` — rename `ToolCard` → `RegisteredToolCard`; runtime state fields + cached `Arc<dyn ToolProvider>`
- `crates/agent-core/src/tools/registry.rs` — `unregister`, `replace`, `set_enabled`, `reload_capability`, `as_tool_set` (feature `rig`)
- `crates/agent-core/src/tools/discovery.rs` — honour `state.json`
- `crates/agent-core/src/tools/providers/chain.rs` — `[chain]` branch using `LlmChainTool`
- `crates/agent-core/src/tools/providers/{mcp,wasm}.rs` — adopt `RegisteredToolCard` rename
- `crates/agent-core/src/context/tenant.rs` — `UserRole`, `role` on `TenantClaims` / `TenantContext`
- `crates/agent-core/src/lib.rs` — re-export new types
- `crates/agent-gateway/src/state.rs` — construct `RegisteredToolAdmin` (with `FilesystemStore`)
- `crates/agent-gateway/src/routes/mod.rs` — wire admin router + OpenAPI tags
- `crates/agent-gateway/src/routes/auth.rs` — set role from `SUPER_ADMIN_EMAILS`
- `crates/agent-gateway/src/routes/capabilities.rs` — filter by `enabled`
- `crates/agent-gateway/src/ui/routes.rs` — `/super-admin/*` routes
- `crates/agent-gateway/src/ui/session.rs` — `role` field
- `crates/agent-gateway/src/ui/view.rs` — admin view structs + `user_role` on `AppView`
- `crates/agent-gateway/src/ui/handlers/app.rs` — pass `user_role`
- `crates/agent-gateway/templates/app.html` — conditional sidebar link
- `apps/backend/Cargo.toml` (workspace) — add `jsonschema` dep; optional `rig` feature

### Updated docs

- `docs/arch.md` — add admin section + super-admin route to file tree
- `docs/verify/verify.md` — add Phase 15: admin UI verification (create/edit/delete/test) + role-gating

---

## Effort & Token Estimate (AI-accelerated)

| Phase | Hours (AI) | Tokens |
|---|---:|---:|
| 0 — auth + role + middleware + audit                | 1.0  | 450   |
| 1 — `PromptTemplate` + `LlmChainTool` (+ Rig impl)  | 2.5  | 1 800 |
| 2 — registry mutability + `as_tool_set` + rename    | 1.5  | 900   |
| 3 — `RegisteredToolStore` + `Validator` + `Admin`   | 2.0  | 2 200 |
| 4 — admin REST routes + utoipa types                | 1.5  | 1 100 |
| 5 — UI handlers + 7 templates + HTMX + flash + test | 3.0  | 2 400 |
| 6 — quotas, allowlist, magic-byte checks            | 0.5  | 300   |
| Tests, clippy, integration, browser verification    | 1.0  | 50    |
| **Total (Phases 0–6 + verification)**               | **13 h** | **~9 200** |
| 7 — optional migration of 3 chain capabilities      | +4 h | +2 500 |

---

## Out of Scope (future phases)

- Per-tenant tool whitelisting and pricing.
- Versioned manifests with rollback ("publish v2, keep v1 enabled until cutover").
- Capability marketplace with publisher signatures.
- LLM-powered manifest authoring ("describe the tool you want, get a draft TOML").
- Hot-swap of running WASM modules without unloading existing fuel state.
- Generic `HashMap<String, Box<dyn ProviderFactory>>` registry (premature; revisit only if a 5th kind is added).
- `parking_lot::RwLock` swap (premature; revisit only if Mutex contention is measured).
- Full SPA frontend (HTMX covers the 95 % case at zero build cost).
