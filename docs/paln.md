```markdown
# ConusAI Platform — Hierarchical Workspace Implementation Plan

**Feature:** Left-sidebar hierarchical workspace (folders + conversations-as-`.md` files)  
**Status:** New (2026-04)  
**Goal:** Enable VS-Code-style project organization where:
- Conversations are real `.md` files stored under tenant-scoped MinIO paths.
- Folders are first-class nodes.
- Agent context is automatically scoped to the selected node + all ancestor folders.
- Zero-code extension, perfect multitenancy, SRP everywhere.

This plan follows **exact project conventions** from `arch.md`, `tenant.md`, and `docs/adr/`.  
It uses only existing dependencies (Qdrant, MinIO/object_store, ULID, TenantContext, safe_path).  
No new heavy crates. Reuses `CapabilityRegistry` discovery patterns for future workspace extensions.

**Estimated AI implementation time:** 9–11 hours / ~2,800 tokens (pure focused changes, no unnecessary features).

---

## 1. Architecture Overview (2026 Best Practice)

**Storage strategy (chosen after community consensus on ClawX / Cursor / Claude Projects patterns):**

| Entity          | Primary Store                  | Human-readable Store                  | Reason |
|-----------------|--------------------------------|---------------------------------------|--------|
| Folders         | Qdrant (`workspaces_{tenant_id}`) | —                                     | Fast tree + semantic search |
| Conversations   | Qdrant node + metadata         | MinIO `tenants/{tenant_id}/workspaces/{virtual-path}/{name}.md` | Editable, Git-exportable, source-of-truth |
| Uploaded files  | Qdrant node                    | Existing MinIO `tenants/{tenant_id}/files/...` | Reuse file-storage capability |

**Context flow (newest Rig-style pattern):**
`ContextBuilder` → loads selected `.md` + ancestor `CONTEXT.md` / `README.md` + semantic siblings → injected as system message.

**Backward compatibility:** Existing flat threads are migrated once via script; old `/v1/threads` endpoints remain functional (alias to root workspace).

---

## 2. Phased Implementation (Minimal, SRP, Reusable)

### Phase 0: Preparation (1 hour / 200 tokens)
**Files to create/modify:**
- `docs/workspace-plan.md` ← **this file**
- `crates/common/src/memory/mod.rs` (re-export new module)
- Update `README.md` → add "Workspace" section with quick-start

**Actions:**
1. Copy existing `memory/thread.rs` patterns.
2. Add `workspace` to `common::prelude`.

**Verification (browser / curl):**
```bash
curl -H "X-Tenant-ID: dev" http://localhost:8080/health
# Expect: capability count unchanged, no breakage
```

### Phase 1: Core Models (1 hour / 350 tokens)
**New file:**
- `crates/common/src/memory/workspace.rs`

**Key types (exact naming from community best practices):**
```rust
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct WorkspaceNode {
    pub id: Ulid,
    pub tenant_id: String,
    pub parent_id: Option<Ulid>,
    pub kind: NodeKind,
    pub name: String,           // e.g. "Sprint-Planning.md" or "Q3 Roadmap"
    pub virtual_path: String,   // "clients/acme/projects/q3/sprint-planning.md"
    pub last_modified: DateTime<Utc>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum NodeKind {
    Folder,
    Conversation,   // always ends with .md
    File,
}
```

**Verification:**
```bash
# After compile
cargo test --package common --test workspace -- workspace::tests
# (add roundtrip serde tests mirroring thread.rs)
```

### Phase 2: WorkspaceStore Trait + Qdrant Impl (3–4 hours / 850 tokens)
**Modify:**
- `crates/common/src/memory/store.rs` → extend `ThreadStore` trait with `WorkspaceStore` (separate trait for SRP)

**New file:**
- `crates/agent-core/src/memory/qdrant_workspace_store.rs` (parallel to `qdrant_store.rs`)

**Trait methods (minimal, reusable):**
```rust
#[async_trait]
pub trait WorkspaceStore: Send + Sync + 'static {
    async fn create_folder(&self, tenant: &TenantContext, parent_id: Option<Ulid>, name: &str) -> Result<WorkspaceNode>;
    async fn create_conversation(&self, tenant: &TenantContext, parent_id: Option<Ulid>, name: &str) -> Result<WorkspaceNode>;
    async fn list_children(&self, tenant_id: &str, parent_id: Option<Ulid>) -> Result<Vec<WorkspaceNode>>;
    async fn get_node(&self, tenant_id: &str, id: Ulid) -> Result<WorkspaceNode>;
    async fn get_ancestors(&self, tenant_id: &str, node_id: Ulid) -> Result<Vec<WorkspaceNode>>;
    async fn get_markdown_content(&self, tenant: &TenantContext, node_id: Ulid) -> Result<String>;
    async fn save_markdown_content(&self, tenant: &TenantContext, node_id: Ulid, content: &str) -> Result<()>;
    async fn move_node(&self, ...); // drag-drop later
}
```

**Qdrant impl:** Reuse 4-dim zero-vector + payload pattern from `QdrantThreadStore`.  
For `create_conversation` → also call MinIO `put` with empty `.md`.

**Verification (curl – works directly in browser dev tools via fetch):**
```bash
# 1. Create folder
curl -X POST http://localhost:8080/v1/workspaces \
  -H "X-Tenant-ID: dev" -H "Content-Type: application/json" \
  -d '{"kind":"folder","parent_id":null,"name":"Clients"}'

# 2. Create conversation (New button flow)
curl -X POST http://localhost:8080/v1/workspaces \
  -H "X-Tenant-ID: dev" -H "Content-Type: application/json" \
  -d '{"kind":"conversation","parent_id":"01J...","name":"Sprint-Planning.md"}'

# 3. List tree (sidebar)
curl -X GET "http://localhost:8080/v1/workspaces/tree?parent_id=01J..." -H "X-Tenant-ID: dev"
```

### Phase 3: MinIO .md Persistence (1 hour / 300 tokens)
**Modify:**
- `crates/agent-gateway/src/state.rs` → add `workspace_store: Arc<dyn WorkspaceStore>` to `AppState`

**Integration:**
- `object_store` client (already in `AppState`) used by `QdrantWorkspaceStore`.
- All paths via `tenant.safe_path("workspaces/...")`.

**Verification:**
```bash
# After creating conversation, manually verify file exists
curl -X GET "http://localhost:9001/minio/conusai/tenants/dev/workspaces/Clients/Sprint-Planning.md" \
  -u minioadmin:minioadmin
# Expect: empty markdown file
```

### Phase 4: ContextBuilder (2 hours / 500 tokens)
**New file:**
- `crates/agent-core/src/memory/context_builder.rs`

**Usage (Rig-friendly):**
```rust
pub struct ContextBuilder<S: WorkspaceStore> { ... }

impl<S: WorkspaceStore> ContextBuilder<S> {
    pub async fn build_for_node(
        &self,
        tenant: &TenantContext,
        node_id: Ulid,
        max_tokens: usize,
    ) -> Result<ConversationContext>;
}
```

**Logic:** ancestor folders → load `CONTEXT.md` / `README.md` + selected `.md` + semantic top-k siblings.

**Modify:**
- `crates/agent-core/src/agent/runtime.rs` → inject `ContextBuilder` into `AgentRuntime`.

**Verification (end-to-end agent):**
```bash
# POST /v1/agent/completions with workspace_node_id
curl -X POST http://localhost:8080/v1/agent/completions \
  -H "X-Tenant-ID: dev" -H "Content-Type: application/json" \
  -d '{
    "messages": [{"role":"user","content":"What is our Q3 goal?"}],
    "workspace_node_id": "01J...",
    "stream": false
  }'
# Expect: response uses content from selected .md + folder context
```

### Phase 5: New API Routes (2 hours / 600 tokens)
**New files (following routes/ pattern):**
- `crates/agent-gateway/src/routes/workspaces.rs` (new router module)
- Update `crates/agent-gateway/src/routes/mod.rs`

**Endpoints (OpenAI-compatible style, minimal):**
- `POST /v1/workspaces` – create folder / conversation
- `GET  /v1/workspaces/tree`
- `GET  /v1/workspaces/{id}/content`
- `PATCH /v1/workspaces/{id}/content` – live save while chatting
- `GET  /v1/workspaces/{id}`

**Context menu support:** same endpoint handles `kind: "folder"`.

**Verification (browser-ready):**
```bash
# Full sidebar tree (copy-paste into browser console or Postman)
curl -X GET http://localhost:8080/v1/workspaces/tree -H "X-Tenant-ID: dev"
```

### Phase 6: Migration & Polish (1 hour / 300 tokens)
**Script:**
- `scripts/migrate_threads_to_workspace.rs` (one-time, idempotent)

**Update:**
- `AppState::from_env()` → register workspace store + run migration if flag set.
- Existing `/v1/threads` endpoints alias to workspace root.

**Final verification script:**
- Extend `scripts/docker-verify.sh` with workspace section (see Phase 7).

---

## 3. Browser / End-to-End Verification Checklist

Run after each phase (or full at the end):

1. **New button (conversation)**
   - Open `http://localhost:8080` (or test UI) → click “+ New” → enter “My-Project.md” → select parent folder.
   - Verify: `.md` file appears in MinIO + node in Qdrant.

2. **Context menu (New Folder)**
   - Right-click any node → “New Folder” → name “2026-Q3”.
   - Verify: tree updates, no path traversal, tenant isolation.

3. **Chat context awareness**
   - Select a conversation inside a folder that has `CONTEXT.md`.
   - Send message referencing folder-level knowledge → agent must use it.

4. **Live save**
   - Chat → edit `.md` via PATCH → refresh tree → content persisted.

5. **Multitenant isolation**
   - Switch `X-Tenant-ID: dev` vs `acme` → different trees, different MinIO prefixes.

6. **Full docker-compose test**
   ```bash
   ./start.sh full
   ./scripts/test_workspace.sh   # new script mirroring docker-verify.sh
   ```

---

## 4. Acceptance Criteria & Quality Gates

- All new code: `cargo clippy -- -D warnings`, `cargo fmt`, 100% test coverage for new modules.
- Zero breaking changes to existing API.
- Tenant safety enforced via `safe_path` everywhere.
- Observability: all new methods instrumented with `#[instrument]`.
- Documentation: update `docs/capabilities.md` with “Workspace” section.

**Next after this plan:**
- Implement Phase 1–2 first (core models + store) → PR for review.
- Then frontend sidebar (separate, not in scope of this backend plan).

**This plan is deliberately minimal** — only what is needed for the exact requested features (New .md button + folder context menu + scoped chat knowledge). Everything else (drag-drop, permissions, etc.) is future ADR.

**Ready to implement?** Say “start phase 1” and I will output the exact files ready to copy-paste into the workspace following every naming, module, and error-handling convention already established in `crates/common` and `agent-core`.
```