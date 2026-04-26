# ConusAI Platform — Hierarchical Workspace Implementation Plan

**Feature:** Left-sidebar hierarchical workspace (folders + conversations as `.md` files) with **private-by-default per-user access** and selective sharing.
**Status:** Updated 2026-04-26 — verified against actual codebase.
**Goal:** VS-Code / Cursor / Claude-Projects style organization where:
- Conversations are real `.md` files in tenant-scoped MinIO paths.
- Folders are first-class nodes with optional `CONTEXT.md` / `README.md`.
- Every node is private to its owner; explicit sharing per node, no inheritance.
- Agent context is auto-scoped to nodes the current user can access.

This plan was **rewritten after a full inventory** of `crates/common`, `crates/agent-core`, `crates/agent-gateway`, `templates/`, `assets/css/style.css`, and `docs/`. Every type, field, route, CSS token, and store pattern referenced below was confirmed to exist (or is explicitly flagged as a required addition).

> **Data policy:** test data is disposable. **No migration scripts, no backward-compat shims.** If a schema changes, drop the Qdrant collection and the MinIO `workspaces/` prefix.

> **Verification policy:** every phase ends by **invoking the `plan-browser-verifier` skill** (`.claude/skills/plan-browser-verifier/SKILL.md`). Phase is not complete until the verdict is `pass` or `pass-with-notes`. On `fail`, fix and re-run before advancing.

---

## 0. Codebase Anchors (verified)

| Concern | Existing artifact |
|---|---|
| Tenant context | `crates/agent-core/src/context/tenant.rs::TenantContext { tenant_id, user_id: Option<String>, plan, workspace_root }` with `safe_path(rel) -> Result<PathBuf>`, `storage_prefix() -> "tenants/{id}/"`, `qdrant_collection(kind) -> "{kind}_{tenant_id}"`, `span_fields()` |
| Tenant middleware | `crates/agent-gateway/src/mw/tenant.rs::ResolvedTenant(pub TenantContext)`. Prod: HS256 JWT `TenantClaims { sub, tenant_id, plan, exp }` → `user_id = Some(sub)`. Dev: `X-Tenant-ID` header → `user_id = None` |
| Errors | `crates/common/src/error.rs::ConusAiError { Config, Capability, Wasm, Mcp, Storage, Api{status,message}, Io, Other }` — **needs `Validation(String)` and `NotFound(String)` variants added** |
| Existing store template | `crates/agent-core/src/memory/qdrant_store.rs::QdrantThreadStore { http: reqwest::Client, base_url: String }` — REST API (not gRPC), 4-dim dummy vectors, payload indices via `PUT /collections/{name}/index` with `field_schema: "keyword"`, scroll via `POST /points/scroll` with filter |
| AppState | `crates/agent-gateway/src/state.rs::AppState { registry, rate_limiter, file_store: Option<Arc<dyn ObjectStore>>, qdrant_url: String, presigned_tokens, thread_store: Arc<dyn ThreadStore>, audit_store: Arc<dyn AuditStore> }` |
| Routes | `crates/agent-gateway/src/routes/mod.rs` — protected routes use `Extension<ResolvedTenant>` |
| Templates | `templates/app.html` (main layout), `partials/composer.html`, `shared/head.html`, `login.html`. Sidebar already exists with "Sections" / "Recents" nav |
| CSS tokens | `--ink/--ink-2/--ink-3`, `--paper/--paper-2/--paper-3`, `--rule`, `--seam`, `--ember/--ember-soft/--ember-glow`, `--success/--danger`, `--font-display/--font-body/--font-mono`, `--t-h1..--t-mono`, `--s-1..--s-8`, `--rail` (260px), `--r-xs..--r-full`, `dur-1..dur-4` |
| Agent runtime | `crates/agent-core/src/agent/runtime.rs::AgentRuntime::for_tenant(model, preamble, registry, tenant)` — system context is injected via the `preamble` string at build time |
| Agent route | `POST /v1/agent/completions` already wired (`routes/agent.rs`) |
| Object store | `object_store = "0.11"` with `aws` feature, exposed as `Arc<dyn ObjectStore>` |
| ADR dir | `docs/adr/` **does not exist yet** — must create alongside ADR 005 |
| Existing thread trait | `crates/common/src/memory/store.rs::ThreadStore` (mirror this shape, do **not** extend it) |

Anything below that names a file/field/method that contradicts this table is the bug — fix the plan, not reality.

---

## 1. Architecture

### Storage layout

| Entity | Index store | Body store | Access rules |
|---|---|---|---|
| Folder | Qdrant `workspaces_{tenant_id}` | — | owner OR `user_id ∈ shared_with` |
| Conversation | Qdrant `workspaces_{tenant_id}` | MinIO `tenants/{tenant_id}/workspaces/{virtual_path}` (`.md`) | owner OR `user_id ∈ shared_with` |
| Uploaded file ref | Qdrant node only | Existing MinIO `tenants/{tenant_id}/files/...` | Same |

### Access model

- Per-node `owner_id: String` and `shared_with: Vec<String>` (user IDs).
- **No inheritance.** Sharing a folder does not share its children. Each node carries its own ACL. Rationale: predictable, easy to reason about, easy to audit.
- All store reads filter Qdrant payload: `tenant_id == X AND (owner_id == U OR shared_with contains U)`.
- Dev mode (`user_id = None`): treat as a single synthetic user `__dev__` so the same logic runs in dev and prod. Filter becomes `owner_id == "__dev__"` automatically.

### Context flow

`ContextBuilder.build_for_node(tenant, node_id, max_tokens)`:
1. `get_ancestors(tenant, node_id)` — already access-filtered.
2. For each ancestor folder, attempt MinIO read of `{path}/CONTEXT.md` then `{path}/README.md`. Missing files are silently skipped.
3. Read selected node body if it is a conversation.
4. Concatenate sections with `\n\n---\n\n`, prefix each with the `virtual_path` as an H2.
5. Truncate from the top (drop most-distant ancestor first) until `~chars/4 ≤ max_tokens`.
6. Return as a `String` — fed to `AgentRuntime::for_tenant(..., preamble = ctx, ...)`.

### Storage invariants

- Every MinIO key built via `tenant.safe_path(format!("workspaces/{virtual_path}"))`. Path traversal returns `Storage`.
- Every Qdrant filter includes the tenant_id condition. There is no global query path.
- Conversation creation order: **MinIO put first, Qdrant upsert second.** A failed Qdrant upsert leaves an orphan `.md` (acceptable; reconcilable). The reverse would leave an index pointing at nothing (returns 500 on read).
- Save order on edit: **MinIO put first, then Qdrant `last_modified` bump.** A failed bump returns `Storage`; the body is already current and the next save will fix the index.

---

## 2. Phased Implementation

Each phase below specifies: files to touch, exact signatures matching project conventions, payload schemas, and a verifier invocation.

### Phase 0 — Preparation

**Files**
- `crates/common/src/error.rs` — add two variants:
  ```rust
  #[error("validation error: {0}")]
  Validation(String),
  #[error("not found: {0}")]
  NotFound(String),
  ```
- `crates/common/src/memory/mod.rs` — add `pub mod workspace;`.
- `docs/adr/005-workspace-access-control.md` — create directory + ADR (template below).
- `README.md` — one-paragraph "Workspace" section pointing at this plan.

**ADR 005 skeleton:**
```markdown
# ADR 005: Workspace Access Control — Private by Default + Selective Sharing
Status: Accepted   Date: 2026-04-26
Decision: Each WorkspaceNode carries owner_id and shared_with: Vec<String>. All store
queries filter on the current TenantContext.user_id (or "__dev__" sentinel in dev mode).
Sharing is explicit per node — no inheritance.
Rationale: predictable, auditable, reuses TenantContext.user_id, Qdrant-native (keyword
indexes on owner_id and shared_with).
```

**Verification — invoke `plan-browser-verifier`**
- Declare: "errors + ADR + module re-export only".
- Drive: `cargo build --workspace` clean. `curl -fsS http://localhost:8088/health` returns 200.
- UI audit: N/A (declare skip).
- Verdict: `pass`.

---

### Phase 1 — Core Models

**File:** `crates/common/src/memory/workspace.rs`

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ulid::Ulid;
use crate::error::{ConusAiError, Result};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind { Folder, Conversation, File }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkspaceNode {
    pub id: Ulid,
    pub tenant_id: String,
    pub owner_id: String,                 // never Option — dev maps None → "__dev__"
    pub parent_id: Option<Ulid>,
    pub kind: NodeKind,
    pub name: String,                     // "Kickoff.md" or "Acme"
    pub virtual_path: String,             // "Clients/Acme/Kickoff.md", no leading slash
    pub last_modified: DateTime<Utc>,
    #[serde(default)]
    pub shared_with: Vec<String>,         // user_ids
    #[serde(default)]
    pub metadata: serde_json::Value,
}

pub fn validate_name(name: &str, kind: NodeKind) -> Result<()> {
    if name.is_empty() || name.len() > 255
        || name.contains('/') || name.contains("..") || name.starts_with('.')
    {
        return Err(ConusAiError::Validation(format!("invalid name: {name:?}")));
    }
    if kind == NodeKind::Conversation && !name.ends_with(".md") {
        return Err(ConusAiError::Validation(
            "conversation names must end in .md".into()));
    }
    Ok(())
}

pub fn join_virtual_path(parent: Option<&str>, name: &str) -> String {
    match parent {
        None | Some("") => name.to_string(),
        Some(p) => format!("{p}/{name}"),
    }
}
```

**Tests** (same file, `#[cfg(test)] mod tests`):
- serde roundtrip (every field).
- `validate_name`: empty / too long / `/` / `..` / leading `.` / missing `.md` for conversation / valid happy path.
- `join_virtual_path`: None, empty, nested.

**Verification — `plan-browser-verifier`**
- Declare: "models only".
- Drive: `cargo test -p common memory::workspace` green.
- UI audit: N/A.
- Verdict: `pass`.

---

### Phase 2 — `WorkspaceStore` Trait + `QdrantWorkspaceStore`

**Files**
- `crates/common/src/memory/store.rs` — add `WorkspaceStore` trait **alongside** `ThreadStore` (do not extend it; SRP).
- `crates/agent-core/src/memory/qdrant_workspace_store.rs` — new file mirroring `qdrant_store.rs` (HTTP REST, `reqwest::Client`).

**Trait:**
```rust
use crate::error::Result;
use crate::memory::workspace::{NodeKind, WorkspaceNode};
use async_trait::async_trait;
use ulid::Ulid;

#[async_trait]
pub trait WorkspaceStore: Send + Sync + 'static {
    async fn create_folder(&self, tenant_id: &str, owner_id: &str,
        parent_id: Option<Ulid>, name: &str) -> Result<WorkspaceNode>;
    async fn create_conversation(&self, tenant_id: &str, owner_id: &str,
        parent_id: Option<Ulid>, name: &str) -> Result<WorkspaceNode>;
    async fn list_accessible_children(&self, tenant_id: &str, user_id: &str,
        parent_id: Option<Ulid>) -> Result<Vec<WorkspaceNode>>;
    async fn get_accessible_node(&self, tenant_id: &str, user_id: &str,
        id: Ulid) -> Result<WorkspaceNode>;
    async fn get_ancestors(&self, tenant_id: &str, user_id: &str,
        node_id: Ulid) -> Result<Vec<WorkspaceNode>>;
    async fn move_node(&self, tenant_id: &str, user_id: &str, node_id: Ulid,
        new_parent_id: Option<Ulid>) -> Result<WorkspaceNode>;
    async fn delete_node(&self, tenant_id: &str, user_id: &str,
        node_id: Ulid) -> Result<()>;          // recursive for folders
    async fn share_node(&self, tenant_id: &str, owner_id: &str, node_id: Ulid,
        with_user_id: &str) -> Result<WorkspaceNode>;       // owner-only
    async fn unshare_node(&self, tenant_id: &str, owner_id: &str, node_id: Ulid,
        with_user_id: &str) -> Result<WorkspaceNode>;       // owner-only
}
```

(Markdown content read/write is on a separate `WorkspaceContentStore` because it needs `ObjectStore` — see below.)

**Body trait:**
```rust
#[async_trait]
pub trait WorkspaceContentStore: Send + Sync + 'static {
    async fn read(&self, tenant: &TenantContext, node: &WorkspaceNode) -> Result<String>;
    async fn write(&self, tenant: &TenantContext, node: &WorkspaceNode, body: &str) -> Result<()>;
}
```

A single `MinioWorkspaceContent { object_store: Arc<dyn ObjectStore> }` impl satisfies it. `read` returns `""` if the object is missing (newly created conversation case).

**Qdrant collection (`workspaces_{tenant_id}`):**

Vector: `{"size": 4, "distance": "Cosine"}` (placeholder, mirrors thread store).

Payload schema (every point):
```json
{
  "id": "01J...", "tenant_id": "dev", "owner_id": "user-123",
  "parent_id": "01J..." | null,
  "kind": "folder" | "conversation" | "file",
  "name": "Kickoff.md", "virtual_path": "Clients/Acme/Kickoff.md",
  "last_modified": "2026-04-26T10:00:00Z",
  "shared_with": ["user-456"], "metadata": {}
}
```

Indexes (created at first use, idempotent — see thread store pattern):
- `tenant_id` keyword
- `owner_id` keyword
- `parent_id` keyword
- `kind` keyword
- `shared_with` keyword (Qdrant indexes array fields automatically; `match` on a single value covers any element)

**Filter helper (used in every accessible read):**
```rust
fn access_filter(tenant_id: &str, user_id: &str, extra: Vec<Value>) -> Value {
    let mut must = vec![ json!({"key":"tenant_id","match":{"value":tenant_id}}) ];
    must.extend(extra);
    json!({
        "must": must,
        "min_should": {
            "conditions": [
                {"key":"owner_id","match":{"value":user_id}},
                {"key":"shared_with","match":{"value":user_id}}
            ],
            "min_count": 1
        }
    })
}
```

> **Qdrant gotcha (verified 2026-04-26):** the REST API expects `min_should` as a **struct**
> with `conditions` + `min_count`, not the integer shorthand. Using `"min_should": 1`
> returns `Format error in JSON body: invalid type: integer 1, expected struct MinShould`.

**Method patterns** (mirror `QdrantThreadStore` exactly):
- `#[instrument(skip(self), fields(tenant_id = %tenant_id, user_id = %user_id))]` on every method.
- HTTP error → `ConusAiError::Storage(format!("workspace: {e}"))`.
- 404 from Qdrant on `points/{id}` → `ConusAiError::NotFound`.
- `delete_node` for folders: scroll children, recurse via worklist (avoid async recursion).
- `share_node` / `unshare_node`: read point, mutate payload `shared_with`, upsert. Verify caller is `owner_id`; otherwise `ConusAiError::NotFound` (do **not** leak existence to non-owners).

**Tests:** `crates/agent-core/tests/workspace_store.rs` against an ephemeral Qdrant. Cover: tenant isolation, owner-only visibility, share-then-list-from-other-user, recursive delete, name validation surfaced via `Validation`.

**Verification — `plan-browser-verifier`**
- Declare: "trait + Qdrant impl + content trait, no HTTP yet".
- Drive: integration test green; `curl http://localhost:6333/collections | jq` shows `workspaces_dev` after the test run.
- UI audit: N/A.
- Verdict: `pass`.

---

### Phase 3 — `AppState` Wiring

**Files**
- `crates/agent-gateway/src/state.rs`:
  ```rust
  pub workspace_store: Arc<dyn WorkspaceStore>,
  pub workspace_content: Arc<dyn WorkspaceContentStore>,
  ```
  `AppState::from_env()` constructs `QdrantWorkspaceStore::new(qdrant_url.clone())` and `MinioWorkspaceContent::new(file_store.clone().expect("file store required for workspace"))`. If `file_store` is `None`, **fail fast at startup** — workspace cannot work without object storage.

**Verification — `plan-browser-verifier`**
- Declare: "store wired into AppState; startup fails clearly without MinIO".
- Drive: `MINIO_ENDPOINT` unset → startup error log mentions workspace requirement. Set, restart → `/health` 200.
- UI audit: N/A.
- Verdict: `pass`.

---

### Phase 4 — `ContextBuilder`

**File:** `crates/agent-core/src/memory/context_builder.rs`

```rust
use std::sync::Arc;
use ulid::Ulid;
use crate::context::tenant::TenantContext;
use crate::memory::workspace::{NodeKind, WorkspaceNode};
use common::error::Result;

pub struct ContextBuilder {
    store: Arc<dyn WorkspaceStore>,
    content: Arc<dyn WorkspaceContentStore>,
}

impl ContextBuilder {
    pub fn new(store: Arc<dyn WorkspaceStore>, content: Arc<dyn WorkspaceContentStore>) -> Self {
        Self { store, content }
    }

    #[instrument(skip(self), fields(tenant_id = %tenant.tenant_id, node_id = %node_id))]
    pub async fn build_for_node(&self, tenant: &TenantContext, node_id: Ulid,
        max_chars: usize) -> Result<String>;
}
```

**Algorithm:** as specified in §1. Implementation notes:
- Resolve effective `user_id`: `tenant.user_id.as_deref().unwrap_or("__dev__")`.
- Folder context probes: try names `["CONTEXT.md", "README.md"]` in order; first hit wins.
- Truncation: count `chars`, drop oldest section while `total > max_chars * 4`. Always keep the selected node body.
- Output starts with `# Workspace context\n` so the agent recognises it.

**Wiring (`crates/agent-gateway/src/routes/agent.rs`):**
- Extend the request body with optional `workspace_node_id: Option<Ulid>` (serde default).
- If present, call `ContextBuilder::build_for_node(&tenant, id, 6000)` and pass the result as the `preamble` argument to `AgentRuntime::for_tenant`. Concatenate with the existing preamble: `format!("{base}\n\n{ctx}")`.
- If absent, behaviour unchanged.

**Tests:** unit-test truncation order with synthetic ancestors.

**Verification — `plan-browser-verifier`**
- Declare: "context injection live; new field `workspace_node_id` on agent route".
- Drive (script):
  1. `POST /v1/workspaces` create folder `clients`.
  2. `PATCH /v1/workspaces/{id}/content` writes `CONTEXT.md` body via the soon-to-exist route, **or** seed MinIO directly via `mc cp` if Phase 5 not done — declare which.
  3. Create conversation `kickoff.md` under it.
  4. `POST /v1/agent/completions` with `workspace_node_id` and a question that requires the seeded fact. Verify response.
- UI audit: N/A.
- Verdict: `pass`.

---

### Phase 5 — HTTP Routes + Sidebar UI

**Files**
- `crates/agent-gateway/src/routes/workspaces.rs` — new module.
- `crates/agent-gateway/src/routes/mod.rs` — register routes.
- `crates/agent-gateway/templates/app.html` — add tree section.
- `crates/agent-gateway/templates/partials/workspace_tree.html` — new partial.
- `crates/agent-gateway/assets/css/style.css` — workspace styles using existing tokens.
- `crates/agent-gateway/assets/js/workspace.js` — new (vanilla, no framework).

**Endpoints:**

| Method | Path | Body | Returns |
|---|---|---|---|
| POST | `/v1/workspaces` | `{kind, parent_id?, name}` | `WorkspaceNode` |
| GET | `/v1/workspaces/tree?parent_id=` | — | `[WorkspaceNode]` |
| GET | `/v1/workspaces/{id}` | — | `WorkspaceNode` |
| GET | `/v1/workspaces/{id}/content` | — | `{content: String}` |
| PATCH | `/v1/workspaces/{id}/content` | `{content}` | `WorkspaceNode` |
| POST | `/v1/workspaces/{id}/move` | `{new_parent_id?}` | `WorkspaceNode` |
| POST | `/v1/workspaces/{id}/share` | `{user_id}` | `WorkspaceNode` |
| POST | `/v1/workspaces/{id}/unshare` | `{user_id}` | `WorkspaceNode` |
| DELETE | `/v1/workspaces/{id}` | — | `204` |

**Handler template** (mirror `routes/audit.rs:20-30`):
```rust
use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use axum::{Extension, Json, extract::{Path, Query, State}, http::StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::instrument;
use ulid::Ulid;

#[derive(Deserialize)]
pub struct CreateBody { pub kind: NodeKind, pub parent_id: Option<Ulid>, pub name: String }

#[instrument(skip(state, tenant, body))]
pub async fn create(
    State(state): State<Arc<AppState>>,
    Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
    Json(body): Json<CreateBody>,
) -> Result<Json<WorkspaceNode>, (StatusCode, Json<Value>)> {
    let owner = tenant.user_id.as_deref().unwrap_or("__dev__");
    let res = match body.kind {
        NodeKind::Folder => state.workspace_store
            .create_folder(&tenant.tenant_id, owner, body.parent_id, &body.name).await,
        NodeKind::Conversation => state.workspace_store
            .create_conversation(&tenant.tenant_id, owner, body.parent_id, &body.name).await,
        NodeKind::File => return Err((StatusCode::BAD_REQUEST,
            Json(json!({"error":"files are created via /v1/files"})))),
    };
    res.map(Json).map_err(map_err)
}

fn map_err(e: ConusAiError) -> (StatusCode, Json<Value>) {
    let code = match &e {
        ConusAiError::Validation(_) => StatusCode::BAD_REQUEST,
        ConusAiError::NotFound(_)   => StatusCode::NOT_FOUND,
        _                           => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (code, Json(json!({"error": e.to_string()})))
}
```

Apply the same shape to every handler. Always:
- `Extension(ResolvedTenant(tenant))` first.
- Rate-limit check on mutating endpoints (`state.rate_limiter.check(&tenant.tenant_id, tenant.plan.rate_limit_rpm())` — see `routes/files.rs:27-35`).
- `#[instrument]` with skipped large args.
- Map `Validation→400`, `NotFound→404`, everything else `→500`.
- For `share`/`unshare`/`delete`/`move`/`PATCH content`, the store enforces owner-only — handler does not need to recheck.

**UI plan (Askama + vanilla JS):**

Add a new sidebar section in `templates/app.html` between "Sections" and "Recents":

```html
<section class="nav-section workspace" aria-labelledby="ws-heading">
  <header class="nav-header">
    <h3 id="ws-heading" class="t-label">Workspace</h3>
    <button type="button" class="icon-btn" data-action="ws-new" aria-label="New folder or conversation">
      <svg><use href="/assets/icons/icons.svg#plus"/></svg>
    </button>
  </header>
  <div id="workspace-tree" class="ws-tree" role="tree" aria-busy="true">
    <div class="ws-skeleton" aria-hidden="true"></div>
  </div>
  <p class="ws-empty" hidden>No items yet. <button type="button" data-action="ws-new" class="link">Create your first folder</button></p>
</section>
```

**JS (`assets/js/workspace.js`) responsibilities:**
- On load: `GET /v1/workspaces/tree` → render root nodes; lazy-load children on `<details>` toggle (`GET /v1/workspaces/tree?parent_id=...`).
- `+` button → `<dialog>` with `<form method="dialog">` for kind/name; submit → `POST /v1/workspaces`; on success, refresh parent.
- Right-click on a node → custom context menu (`<ul role="menu">` positioned at cursor, dismissed on outside click / Escape) with: New folder, New conversation, Rename (F2), Share…, Move…, Delete.
- Selecting a conversation → fetches `/content`, shows in editor pane (existing composer area gains a small `data-node-id` attribute), and every subsequent send to `/v1/agent/completions` includes `workspace_node_id`.
- Live-save: editor `blur` → `PATCH /content`. Debounce 800ms during typing.
- Share dialog → `POST /share` with target `user_id`. Show current `shared_with` list with "Remove" buttons (→ `/unshare`).
- Drag-and-drop tree reorder → `POST /move`.
- Keyboard map: `↑/↓` move focus, `→` expand, `←` collapse, `Enter` open, `F2` rename, `Delete` delete (with confirm), `Ctrl/Cmd+N` new conversation in current folder.

**CSS additions** (use existing tokens, no new colors):
```css
.ws-tree { --indent: var(--s-3); display:flex; flex-direction:column; gap:2px; }
.ws-tree [role="treeitem"] { display:flex; align-items:center; gap:var(--s-1);
  padding:var(--s-1) var(--s-2); border-radius:var(--r-xs); color:var(--ink-2);
  font:var(--t-body); cursor:default; }
.ws-tree [role="treeitem"]:hover { background:var(--paper-2); color:var(--ink); }
.ws-tree [aria-current="page"] { background:var(--ember-soft); color:var(--ink); }
.ws-tree [role="treeitem"]:focus-visible { outline:2px solid var(--ember); outline-offset:1px; }
.ws-tree details > summary { list-style:none; }
.ws-tree details[open] > summary svg.chev { transform: rotate(90deg); }
.ws-skeleton { height:120px; background:linear-gradient(90deg,var(--paper-2),var(--paper-3),var(--paper-2));
  background-size:200% 100%; animation: ws-shimmer 1.2s linear infinite; border-radius:var(--r-xs); }
@keyframes ws-shimmer { from { background-position:200% 0; } to { background-position:-200% 0; } }
```

**UI checklist (the verifier will enforce):**
- Visible focus rings on every interactive element.
- Empty state with explicit call-to-action.
- Skeleton loader (not spinner) while tree loads.
- Toast or `<output role="status" aria-live="polite">` for mutation errors.
- `aria-expanded` on folder summaries; `aria-current="page"` on selected node.
- Keyboard-only navigation works for every action.
- Sidebar collapses cleanly at <768px (existing layout already supports this — verify it still does).
- WCAG AA contrast against both `paper` and `forge` themes.

**Verification — `plan-browser-verifier` (the big one)**
- Declare every new route + every UI element.
- Confirm dev server fresh (`cargo build -p agent-gateway && pkill agent-gateway; CONUSAI_SERVER__PORT=8088 target/debug/agent-gateway &`).
- Drive (Chrome MCP):
  1. Open `http://localhost:8088`. Wait ≥1s. Screenshot empty state. Confirm CTA visible.
  2. Click `+` → dialog opens, focus traps inside, Esc closes. Create folder `Clients`. Tree updates.
  3. Right-click `Clients` → context menu shown at cursor; create `New folder Acme`. Verify nesting.
  4. Inside `Acme`, create conversation `Kickoff.md`. Parallel-curl MinIO to confirm `tenants/dev/workspaces/Clients/Acme/Kickoff.md` exists.
  5. Type a message in composer with `Kickoff.md` selected; in Chrome devtools confirm request payload contains `"workspace_node_id":"01J..."`.
  6. Drag `Kickoff.md` onto root; tree reflects move; `virtual_path` updated server-side (re-fetch and check).
  7. Open Share dialog on `Acme`; add `user-other`; switch JWT to `user-other`; verify `Acme` now visible to that user but `Kickoff.md` (not shared) is not.
  8. Delete `Acme` (recursive confirm). Verify gone from Qdrant scroll AND MinIO `ls`.
  9. Tab through entire sidebar — every node reachable, focus order matches DOM order.
  10. Resize to 375px wide — sidebar collapses, hamburger works.
- UI audit: full checklist.
- Verdict: `pass` or `pass-with-notes`. On `fail`, fix and re-drive the entire sequence.

---

### Phase 6 — Per-Node Thread Binding (the "every .md is its own chat" feature)

**Why this matters.** Phase 5 ships the workspace as a *document* tree. Conversation
history, however, lives in a separate `Thread` (Qdrant `threads_{tenant_id}`) addressed
by a free-floating `thread_id`. Without binding, switching `Kickoff.md → Notes.md` in
the sidebar leaves `activeThreadId` unchanged — the user appears to "continue the same
chat" across files. To match Cursor / Claude Projects expectations, **each conversation
node must own a persistent thread**, lazily created on first message and rehydrated on
re-selection.

**Design** (chosen after weighing alternatives — see §5 below):
- Store `thread_id` inside `WorkspaceNode.metadata` (existing `serde_json::Value` slot).
  No schema migration; Qdrant payload is already free-form JSON.
- The binding is *server-resolved* on the chat path. The browser only sends
  `workspace_node_id`. The server looks up `metadata.thread_id`; if missing it creates a
  thread and writes the binding back. Atomic from the client's POV — one round-trip.
- The first-message thread create is wrapped in an idempotent helper so a refresh-storm
  cannot fork two threads onto the same node (re-read-then-decide pattern, mirrors how
  `ensure_collection` is implemented).

**Files**
- `crates/common/src/memory/store.rs` — extend `WorkspaceStore`:
  ```rust
  /// Persist `thread_id` into `metadata.thread_id`. Idempotent.
  /// Returns the updated node. Owner-only check: callers must have already
  /// resolved access via `get_accessible_node`.
  async fn bind_thread(
      &self,
      tenant_id: &str,
      node_id: Ulid,
      thread_id: &str,
  ) -> anyhow::Result<WorkspaceNode>;
  ```
- `crates/agent-core/src/memory/qdrant_workspace_store.rs` — implement `bind_thread`:
  read-modify-write the point payload; merge into `metadata` rather than overwrite.
- `crates/agent-gateway/src/routes/agent.rs::build_ctx` — *before* loading thread
  history, resolve effective `thread_id`:
  ```rust
  let effective_thread_id = match (req.thread_id.clone(), req.workspace_node_id.as_deref()) {
      (Some(tid), _) => Some(tid),                           // explicit wins
      (None, Some(node_id_str)) => {
          let node_id: Ulid = node_id_str.parse()?;
          let node = state.workspace_store
              .get_accessible_node(&tenant.0.tenant_id, effective_user_id(...), node_id).await?;
          match node.metadata.get("thread_id").and_then(|v| v.as_str()) {
              Some(tid) => Some(tid.to_string()),
              None => {
                  let t = state.thread_store.create(&tenant.0.tenant_id, vec![]).await?;
                  let _ = state.workspace_store
                      .bind_thread(&tenant.0.tenant_id, node_id, &t.id).await;
                  Some(t.id)
              }
          }
      }
      (None, None) => None,
  };
  ```
  Then everything downstream uses `effective_thread_id` exactly as it uses
  `req.thread_id` today. The SSE stream already echoes `thread_id` back to the client
  (`app.js:225` captures it into `activeThreadId`), so no change is required there.

- `crates/agent-gateway/assets/js/workspace.js::selectConversation` — extend the
  existing `ws:select` event with `thread_id` from `node.metadata`:
  ```javascript
  const threadId = node?.metadata?.thread_id ?? null;
  document.dispatchEvent(new CustomEvent("ws:select", {
      detail: { nodeId: node.id, node, threadId }
  }));
  ```
  When `restoreNodeFromUrl` ran a `GET /v1/workspaces/{id}` it already has the metadata.
  When the user clicks a tree leaf we *don't* have metadata in hand (the tree fetch
  is `list_accessible_children` which returns full nodes — confirm) — if not, do a
  one-off `GET /v1/workspaces/{id}` here.

- `crates/agent-gateway/assets/js/app.js` — listen for `ws:select`:
  ```javascript
  document.addEventListener("ws:select", async (e) => {
      const { threadId } = e.detail;
      activeThreadId = threadId;        // null = fresh thread will be created server-side
      messagesEl.innerHTML = "";        // clear old conversation
      if (!threadId) { showGreeting(); return; }
      showChatView();
      try {
          const res = await fetch(`/v1/threads/${threadId}/messages`);
          if (!res.ok) return;
          const { data } = await res.json();
          for (const m of data) {
              appendMessage(m.role === "assistant" ? "ai" : m.role, m.content);
          }
      } catch (_) {}
  });
  ```

**Failure modes & how the design handles them**
- *User refreshes mid-create:* node has no `thread_id` yet, server creates one,
  binds it. Race window is short (single Qdrant round-trip), but if two requests
  collide the second binding overwrites the first and the older empty thread is
  orphaned — acceptable.
- *Node deleted while open:* `bind_thread` returns `NotFound`; chat still works
  (server falls through to "no binding" branch and a transient thread is created
  for that single turn — never persisted anywhere visible).
- *Shared node:* the thread is owned by the original creator's tenant, but
  `tenant_id` is shared across both users (single-tenant model). Both users append
  to the same thread. **This is intentional** — shared `.md` = shared conversation.
  Document this in the share dialog tooltip in Phase 5.

**Tests** (`crates/agent-gateway/tests/workspace_thread_binding.rs`):
1. Create conversation node, send chat with `workspace_node_id`, no `thread_id` →
   response `thread_id` is non-null. Re-fetch node, `metadata.thread_id` matches.
2. Send a second message with the *same* `workspace_node_id`, no `thread_id` →
   `thread_id` matches the first. Thread now has 4 messages (2 user + 2 assistant).
3. Send with explicit `thread_id` AND `workspace_node_id` → explicit wins (no rebind).
4. Two sibling conversations created in the same folder → independent thread_ids,
   no cross-contamination of message history.

**Verification — `plan-browser-verifier`**
- Drive (Chrome MCP):
  1. Reset Qdrant + MinIO. Create folder `Projects`. Inside, create `alpha.md`,
     `beta.md`, `gamma.md`.
  2. Select `alpha.md`. Send "remember the word ORANGE." → response acknowledges.
  3. Select `beta.md`. Send "what word did I just tell you?" → response must NOT
     contain "ORANGE" (different thread). Send "remember the word PURPLE."
  4. Re-select `alpha.md`. Verify message list re-renders with the ORANGE turn.
     Send "what word?" → response contains "ORANGE", NOT "PURPLE".
  5. Re-select `beta.md`. Send "what word?" → "PURPLE", not "ORANGE".
  6. Hard refresh page; URL `?ws=<gamma_id>` restores selection AND empty thread.
  7. DevTools: `GET /v1/workspaces/{alpha_id}` → `metadata.thread_id` populated.
- Verdict: `pass`.

---

### Phase 7 — Multi-Layer Context Composition (2026 RAG hygiene)

**Why.** Phase 4's `ContextBuilder` only stitches ancestor `CONTEXT.md` files. Modern
agent loops (per the OpenAI Memory paper, Anthropic's Projects writeup, and the
2026 LangGraph "long-form memory" guidance) compose context in **layers** with explicit
budgets per layer so no single source can crowd out the others.

**Layered context budget** (target: ≤ 60% of model's context window so tools + the
turn itself fit):
| Layer | Default budget | Source |
|---|---|---|
| L1 — System role/persona | 1k tokens | hard-coded preamble in `AgentRuntime` |
| L2 — Workspace ancestors (`CONTEXT.md`) | 4k tokens | `ContextBuilder` (existing) |
| L3 — Selected `.md` body | 2k tokens | `ContextBuilder` (existing) |
| L4 — Thread summary (rolling) | 1k tokens | `Thread.summary` (existing field) |
| L5 — Recent thread messages (verbatim) | 8k tokens | `thread_store.messages` |
| L6 — Tool result history | 4k tokens | tool message turns inside `messages` |
| Current turn | rest | user message |

**What changes**
- `ContextBuilder::build_for_node` gains an explicit `BudgetConfig` instead of a flat
  `max_chars`. Per-layer truncation strategy:
  - L2: drop oldest ancestor first (already implemented).
  - L3: head-truncate the `.md` body (keep the tail = most recent edits).
  - L5: keep last N message *pairs* until budget hit; oldest pairs become input to L4
    summarisation (deferred to a background task — see "Auto-summarisation" below).
- `agent.rs::build_ctx` reads thread summary AND messages, and respects L5 budget
  by keeping last N messages whose total token count ≤ L5 budget. Older messages are
  dropped; the dropped slice is enqueued for summarisation if not already summarised.

**Auto-summarisation** (Phase 7.b, optional but recommended)
- New `crates/agent-core/src/memory/summariser.rs`: tokio task triggered when a
  thread crosses 16k tokens. Calls Anthropic with a "summarise the following
  conversation in 800 tokens, preserving facts, decisions, and open questions"
  preamble, writes result via `thread_store.set_summary`.
- `build_ctx` already prepends `[Conversation summary: ...]` when present
  (`agent.rs:160`). Just need to *generate* one.

**Citations** (Phase 7.c)
- When the agent quotes from `.md` content, prefix with `[ws:<virtual_path>:Lstart-Lend]`
  via a system instruction in `ContextBuilder`'s output. The UI renders these as
  clickable badges that scroll to the source line in the document panel (Phase 8).

**Tests**
- Unit: `BudgetConfig` enforcement — feed 30k synthetic ancestor text, assert L2
  output ≤ 4k tokens.
- Integration: send 50 messages on a single node, verify L5 truncates and L4 summary
  appears.

**Verification — `plan-browser-verifier`**
- Seed `Clients/Acme/CONTEXT.md` with 5k-token brief.
- Open `kickoff.md`, run a 25-turn conversation. After turn 25, devtools network tab:
  the request body to Anthropic shows `system` includes the brief AND the rolling
  summary AND the last ~8k of message history; older turns are NOT verbatim.
- Verdict: `pass`.

---

### Phase 8 — Live Document Mode (agent writes back to `.md`)

**Why.** A workspace where the agent only *reads* `.md` files is half the value. The
2026 community pattern (cf. Cursor "Composer", Cline "memory bank", Claude Code's own
`CLAUDE.md` workflow) is **bidirectional**: the agent can append decisions, write
sub-files, or update notes during a turn. This makes the workspace a self-curating
knowledge base instead of a write-once briefing folder.

**Mechanism** — expose three new tools to the agent runtime when `workspace_node_id`
is set on the request:

| Tool name | Args | Effect |
|---|---|---|
| `workspace__read_file` | `{path}` | Read any `.md` in the *current node's ancestor scope* |
| `workspace__append_section` | `{path, heading, body}` | Append `## heading\n{body}` to file |
| `workspace__create_file` | `{parent_path, name, body}` | Create new `.md` under a sibling/child folder |

**Safety rails** (non-negotiable):
- Tool definitions are emitted **only when** `workspace_node_id` is present AND the
  caller has write access (i.e. `owner_id == user_id`). Shared-but-not-owned nodes get
  read-only tools.
- Path argument is resolved through `tenant.safe_path(format!("workspaces/{path}"))`
  and rejected if it escapes the *ancestor scope* of the current node (the same set
  `ContextBuilder` walks). No editing arbitrary tenant files.
- Every write goes through `WorkspaceContentStore.write` → `WorkspaceStore.bump_last_modified`,
  emitting a structured audit log entry: `{tool, node_id, path, bytes_written, user_id}`.
- A new SSE event `workspace_change` is emitted to the browser so the document panel
  re-fetches and re-renders without a page reload.

**Files**
- `crates/agent-core/src/capabilities/builtin/workspace_tools.rs` — implement as a
  built-in capability (not a WASM module — it needs `Arc<dyn WorkspaceStore>`).
  Mirror the shape of the existing `tool_executor` invocation path.
- `crates/agent-gateway/src/routes/agent.rs` — when building `tools` Vec, conditionally
  include the workspace tools based on request payload.

**UX contract**
- The document panel shows a subtle "Live edit" indicator while the agent's tool call
  is pending; on `workspace_change`, the panel diff-highlights the new lines for 4s.

**Verification — `plan-browser-verifier`**
- "Add a TODO section to this file with three actionable items" → agent calls
  `workspace__append_section`, file content updates live in the right panel.
- Path-escape attempt: send a crafted prompt asking for `../../../etc/passwd` →
  tool returns `Error: path outside scope`, agent recovers gracefully.
- Verdict: `pass`.

---

### Phase 9 — Workspace as Agent Toolkit (search + lateral context)

**Why.** With dozens of `.md` files, the agent can't be *handed* every relevant doc
through `ContextBuilder`. It must *search* its own workspace, the same way a human
opens VS Code's quick-find. This is the 2026 standard for long-horizon agents
(SWE-bench top entries, Cursor's "@workspace", Cline's RAG-on-demand).

**Tools added on top of Phase 8:**

| Tool | Args | Effect |
|---|---|---|
| `workspace__search` | `{query, kind?, limit?}` | BM25 + vector hybrid over node names + bodies; returns `[{path, snippet}]` |
| `workspace__list_tree` | `{root_path?, depth?}` | Returns a compact JSON tree (no bodies) so the agent can pick targets before reading |

**Implementation notes**
- Vectors in `workspaces_{tenant}` are still 4-dim placeholders. Phase 9 *replaces*
  them with real embeddings (cohere/voyage/whichever) — separate ADR (`006-workspace-embeddings.md`)
  will pick the model. Until that ADR lands, `workspace__search` falls back to
  payload-text scan (slow but correct), gated behind a feature flag
  `workspace_search_enabled = false` by default.
- BM25 stored as a separate sparse vector inside the same Qdrant collection (Qdrant
  ≥ 1.10 supports hybrid). This avoids a second store.
- All search results pass through the same `access_filter` as everything else.

**Verification** — see ADR 006 (deferred).

---

## 5. Architecture Trade-offs Considered

| Question | Chosen | Rejected | Why |
|---|---|---|---|
| Where to store thread binding? | `WorkspaceNode.metadata` JSON | New `node_threads` collection; rename `Thread.metadata.node_id` | Metadata field already exists, no schema churn, single-document atomic update, no second source of truth to keep in sync. |
| One thread per node, or one thread shared across siblings? | One per node | One per folder; one global per session | Predictable mental model = one `.md` ⇔ one conversation. Folder-scoped would surprise users when context bleeds. |
| Auto-create thread on node *create*, or on first *message*? | First message | On create | Avoids orphan threads (most created nodes never get a chat). Lazy = cheaper. |
| Resolve binding client-side or server-side? | Server-side | Client passes both `workspace_node_id` and looked-up `thread_id` | One round-trip. No client retry logic. Server is the only writer of `metadata.thread_id` so race window is bounded. |
| What happens when a shared node receives messages from two users? | Append to same thread (current behaviour) | Per-user thread fork | Shared `.md` is collaborative by design; forking would surprise the owner. Author of each message is captured in `Message.role` (future: add `Message.author_id` — out of scope). |
| Tool augmentation surface (Phase 8/9) | Built-in capability with workspace store dependency | WASM capability with stdio shim | WASM can't hold an `Arc<dyn WorkspaceStore>`. Built-in is the right boundary; WASM is for sandbox-required code. |

---

## 3. Acceptance Criteria

- `cargo fmt --all -- --check` clean.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo test --workspace` green (incl. new workspace + access-control tests).
- All new public methods carry `#[instrument]` with `tenant_id` and `user_id` fields.
- Every MinIO key built via `tenant.safe_path` — no raw `format!` paths into `object_store`.
- Every Qdrant query includes `tenant_id` AND access (`owner_id` OR `shared_with`) filter.
- No `.unwrap()` / `.expect()` in route handlers (only at startup wiring).
- Verifier verdict `pass` recorded for every phase.
- README updated.

## 4. Out of Scope (deliberately deferred)

- Cross-tenant sharing.
- Group / role-based ACLs (only individual user IDs for now).
- Real-time collaboration / CRDT.
- Workspace-wide semantic search (vectors are placeholder; future ADR).
- Versioning / Git sync of `.md` files.
- Uploading files directly into the tree (uploads stay on `/v1/files`; tree only references them via a separate ADR).
