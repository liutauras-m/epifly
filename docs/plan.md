# ConusAI Platform — v0.3.3 Implementation Plan
## Zero-Core-Touch Mode + Self-Registering Generic Agent Capabilities

**Version:** v0.3.3  
**Branch:** `feat/v0.3.3-zero-core-touch`  
**Status:** Ready for AI implementation  
**Goal:** Ship the infrastructure that lets any new capability self-register over HTTP — no Rust rebuild, no Cargo changes, no platform restart — and validate it with a live `current-time` MCP service.

---

## Codebase Baseline (v0.3.2)

All of the following already exist and must NOT be changed unless a phase explicitly modifies them:

| Component | Location | Status |
|-----------|----------|--------|
| `CapabilityProvider` trait | `agent-core/src/tools/provider.rs` | ✅ complete |
| `CapabilityCard` | `agent-core/src/tools/card.rs` | ✅ complete |
| `ToolRegistry` | `agent-core/src/tools/registry.rs` | ✅ complete |
| `SemanticCapabilityRouter` | `agent-core/src/tools/semantic_router.rs` | ✅ complete |
| `DynamicPromptCapability` | `agent-core/src/chains/dynamic_prompt.rs` | ✅ complete |
| `CapabilitySpecFactory` | `agent-core/src/tools/providers/capability_spec.rs` | ✅ complete |
| `PgVectorStore` | `agent-core/src/vector_store/postgres.rs` | ✅ complete |
| `EmbeddingService` | `agent-core/src/indexing/embedding_service.rs` | ✅ complete |
| `RealtimeService` | `agent-core/src/realtime/mod.rs` | ✅ LISTEN/NOTIFY wired |
| `McpAdapter` / `McpProvider` | `agent-core/src/tools/providers/mcp.rs` | ✅ file-based only |
| `/admin/capabilities/*` CRUD | `agent-gateway/src/routes/admin_capabilities.rs` | ✅ TOML/WASM |
| `RouterQuotaLayer` | `agent-gateway/src/mw/router_quota.rs` | ✅ complete |
| `AppState` | `agent-gateway/src/state.rs` | ✅ all fields |
| DB migrations | `common/migrations/` | ✅ 7 migrations |

**What is missing (gaps this plan closes):**

1. No `/admin/capabilities/register` endpoint accepting JSON manifests from external services
2. No `RemoteMcpCapability` — dynamic MCP provider constructed from a JSON payload, not a TOML file
3. No `tenant_scope` field — capabilities cannot be scoped to specific tenant IDs
4. `RealtimeService` hot-reload not wired to `CapabilitySpecFactory::reload_one()` at startup
5. No `ArtifactBridge` — tool outputs with file artifacts are not persisted to workspace nodes
6. No `services/` directory — no example of a self-registering external capability
7. `ToolKind` enum missing `RemoteMcp` variant

---

## Phase 0 — Readiness Verification (pre-flight, 0 new code)

**Before starting** run these commands and confirm all pass:

```bash
# 1. Compile check
cargo check -p agent-core -p agent-gateway

# 2. Full test suite
cargo test --workspace

# 3. Docker infra up
docker compose --profile infra up -d

# 4. Run migrations
make db-migrate

# 5. Smoke-test admin route
JWT=$(curl -s -X POST http://localhost:8080/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email":"admin@test.local","password":"dev"}' | jq -r .token)
curl -H "Authorization: Bearer $JWT" http://localhost:8080/admin/capabilities | jq length
```

Expected: all tests pass, admin route returns array (possibly empty).

---

## Phase 1 — `RemoteMcpCapability` + `ToolKind::RemoteMcp` (2 AI-hours)

**Principle:** External MCP services must be invokable without a TOML file on disk. The existing `McpProvider` reads its endpoint from `card.manifest.config["endpoint"]` which requires a file. We need a variant that stores all state in the DB row payload.

### 1.1 — Add `RemoteMcp` to `ToolKind` enum

**File:** `apps/backend/crates/agent-core/src/tools/manifest.rs`

Add to the `ToolKind` enum:
```rust
/// External MCP service registered via JSON (no TOML file on disk).
#[serde(rename = "remote_mcp")]
RemoteMcp,
```

### 1.2 — Create `RemoteMcpCapability`

**New file:** `apps/backend/crates/agent-core/src/tools/providers/remote_mcp.rs`

```rust
//! `RemoteMcpCapability` — dynamically-registered MCP provider.
//!
//! Unlike `McpProvider` (file-based, kind=Mcp), this type is constructed
//! entirely from a JSON registration payload and requires no TOML on disk.
//! It is created by `CapabilityRegistrar::register_json()` and stored in
//! `capability_specs` with strategy = "remote_mcp".

use crate::context::tenant::TenantContext;
use crate::tools::manifest::ToolManifest;
use crate::tools::mcp_adapter::McpAdapter;
use crate::tools::provider::CapabilityProvider;
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

pub struct RemoteMcpCapability {
    manifest: ToolManifest,
    endpoint: String,
}

impl RemoteMcpCapability {
    pub fn new(manifest: ToolManifest, endpoint: String) -> Arc<Self> {
        Arc::new(Self { manifest, endpoint })
    }
}

#[async_trait]
impl CapabilityProvider for RemoteMcpCapability {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        _tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        let adapter = McpAdapter::new(&self.endpoint)
            .map_err(|e| anyhow::anyhow!("MCP adapter error: {e}"))?;
        adapter
            .call_tool(tool_name, input.clone())
            .await
            .map_err(|e| anyhow::anyhow!("MCP call_tool error: {e}"))
    }
}
```

**Register in providers/mod.rs:**
```rust
pub mod remote_mcp;
```

### 1.3 — Add `remote_mcp` strategy to `CapabilitySpecFactory::row_to_provider`

**File:** `apps/backend/crates/agent-core/src/tools/providers/capability_spec.rs`

In `row_to_provider()`, add a new arm to the `match row.strategy.as_str()` block:

```rust
"remote_mcp" => (ToolKind::RemoteMcp, None),
```

And in the final provider construction match, add:
```rust
ToolKind::RemoteMcp => {
    let endpoint = row.payload["endpoint"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("remote_mcp spec '{}' missing payload.endpoint", cap_name))?
        .to_string();
    providers::remote_mcp::RemoteMcpCapability::new(manifest, endpoint)
}
```

**Tests to add** in the same file's `#[cfg(test)]` block:
- `remote_mcp_provider_forwards_to_adapter` — mock endpoint, verify `invoke` calls `call_tool`

---

## Phase 2 — `tenant_scope` for Capability-Level Tenant Isolation (2 AI-hours)

**Principle:** A capability should be visible only to the tenants it was registered for. `"global"` means all tenants. The filter must be applied at `SemanticCapabilityRouter::select()` time and at `/v1/capabilities` list time.

### 2.1 — Migration: add `tenant_scope` to `capability_specs`

**New file:** `apps/backend/crates/common/migrations/20260507000300_capability_tenant_scope.up.sql`

```sql
ALTER TABLE capability_specs
    ADD COLUMN IF NOT EXISTS tenant_scope TEXT[] NOT NULL DEFAULT '{}';
-- Empty array = global (visible to all tenants).
-- Non-empty = visible only to listed tenant IDs.
COMMENT ON COLUMN capability_specs.tenant_scope IS
    'Empty = global. Non-empty = restrict to these tenant IDs.';

CREATE INDEX IF NOT EXISTS capability_specs_scope_idx
    ON capability_specs USING GIN (tenant_scope);
```

**Also update** `docker/init/02-schema.sql` to add `tenant_scope TEXT[] NOT NULL DEFAULT '{}'` to the `capability_specs` CREATE TABLE definition, plus the GIN index.

### 2.2 — Add `tenant_scope` to `CapabilitySpecRow`

**File:** `apps/backend/crates/agent-core/src/tools/providers/capability_spec.rs`

Add to `CapabilitySpecRow`:
```rust
tenant_scope: Vec<String>,
```

### 2.3 — Add `tenant_scope` to `CapabilityCard` / `ToolManifest`

**File:** `apps/backend/crates/agent-core/src/tools/manifest.rs`

Add to `ToolManifest`:
```rust
/// Empty = global (all tenants). Non-empty = only these tenant IDs see this capability.
#[serde(default)]
pub tenant_scope: Vec<String>,
```

**File:** `apps/backend/crates/agent-core/src/tools/card.rs`

Add helper to `CapabilityCard`:
```rust
/// Returns true if this capability is visible to `tenant_id`.
/// An empty scope means global (always visible).
pub fn is_visible_to(&self, tenant_id: &str) -> bool {
    let scope = &self.manifest.tenant_scope;
    scope.is_empty() || scope.iter().any(|t| t == tenant_id)
}
```

### 2.4 — Enforce scope in `SemanticCapabilityRouter::select()`

**File:** `apps/backend/crates/agent-core/src/tools/semantic_router.rs`

After ANN hits are collected and distance-filtered, add a scope filter:
```rust
// Filter by tenant_scope if tenant is known.
if let Some(t) = tenant {
    let registry = self.registry.lock().unwrap();
    cap_names.retain(|name| {
        registry.get(name)
            .map(|card| card.is_visible_to(&t.tenant_id))
            .unwrap_or(false)
    });
}
```

Also enforce in `ToolRegistry::search_by_namespace()` — add `tenant_id: Option<&str>` parameter and filter cards by `is_visible_to`.

### 2.5 — Enforce scope at `/v1/capabilities` list route

**File:** `apps/backend/crates/agent-gateway/src/routes/` (capabilities listing handler)

When building the list response, filter by `card.is_visible_to(&tenant.tenant_id)`.

**Tests:**
- Unit test in `card.rs`: `is_visible_to_global`, `is_visible_to_specific_tenant`, `is_not_visible_to_other_tenant`
- Unit test in `semantic_router.rs`: `select_respects_tenant_scope`

---

## Phase 3 — `POST /admin/capabilities/register` Self-Registration Endpoint (2 AI-hours)

**Principle:** External services call this endpoint with a JSON manifest on startup. No TOML file, no WASM binary, no disk access. The registration atomically: validates → persists to `capability_specs` → embeds → inserts into registry.

### 3.1 — Request/Response types

**File:** `apps/backend/crates/agent-gateway/src/routes/admin_capabilities.rs`

Add new request type:
```rust
/// JSON manifest posted by external self-registering capability services.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct CapabilityRegisterRequest {
    /// Unique ID, e.g. "media.time.current-time" — stored as capability_id.
    pub capability_id: String,
    /// Human name, used as tool_name in capability_specs.
    pub name: String,
    /// Dot-separated namespace, e.g. "media.time".
    pub namespace: String,
    pub description: String,
    pub version: String,
    /// Must be "remote_mcp" for self-registering MCP services.
    pub kind: String,
    /// MCP server endpoint URL (required when kind = "remote_mcp").
    pub endpoint: Option<String>,
    /// Tool definitions (name + description + JSON Schema).
    pub tools: Vec<ToolDefJson>,
    /// Empty = global. Non-empty = only these tenant IDs.
    #[serde(default)]
    pub tenant_scope: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct ToolDefJson {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

fn default_true() -> bool { true }
```

### 3.2 — Handler implementation

Add route `POST /admin/capabilities/register` in `admin_capabilities.rs`:

```rust
pub async fn register_capability(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CapabilityRegisterRequest>,
) -> Result<impl IntoResponse, HttpError>
```

**Implementation steps inside the handler:**

1. **Validate** — `capability_id` must match pattern `^[a-z][a-z0-9._-]{1,127}$`; `kind` must be `"remote_mcp"` (only supported kind for self-registration); `endpoint` required when kind = `"remote_mcp"`; `tools` must be non-empty.
2. **Upsert `capability_specs` row** — use `INSERT ... ON CONFLICT (namespace, tool_name) DO UPDATE` so re-registration is idempotent:
   ```sql
   INSERT INTO capability_specs
       (id, namespace, tool_name, description, input_schema, output_schema,
        strategy, payload, tags, tenant_scope, enabled)
   VALUES ($1, $2, $3, $4, $5, NULL, 'remote_mcp',
           jsonb_build_object('endpoint', $6), $7, $8, $9)
   ON CONFLICT (namespace, tool_name)
   DO UPDATE SET
       description  = EXCLUDED.description,
       payload      = EXCLUDED.payload,
       tags         = EXCLUDED.tags,
       tenant_scope = EXCLUDED.tenant_scope,
       enabled      = EXCLUDED.enabled,
       updated_at   = now()
   RETURNING id
   ```
3. **Build provider** — call `CapabilitySpecFactory::row_to_provider()` with the synthesised row (no DB re-read needed — the row data is already in memory from step 2).
4. **Embed + upsert vector** — call `state.embedding_service.embed_query(&embedding_text)` then `state.vector_store.upsert_capability_embedding_full(...)`.
5. **Register in-process** — `state.registry.lock().unwrap().register(card.with_provider(provider))`.
6. **Invalidate router cache** — `state.semantic_router.invalidate_all().await`.
7. **Audit log** — `state.audit_store.append(AuditEvent::new("system", "capability.register").with_metadata(json!({...})))`.
8. **Return** `201 Created` with `{ "capability_id": "...", "registered": true }`.

### 3.3 — Wire route in router

**File:** `apps/backend/crates/agent-gateway/src/routes/mod.rs`

Add `register_capability` to the admin router alongside existing admin capability routes.

### 3.4 — Tests

Add integration test in `admin_capabilities.rs` `#[cfg(test)]` block:
- `register_new_capability_201` — POST valid JSON, assert 201, assert registry contains the capability
- `register_idempotent_on_conflict` — POST same capability_id twice, assert 200 on second call (not 409)
- `register_rejects_unknown_kind` — POST with `kind = "unknown"`, assert 400
- `register_rejects_missing_endpoint_for_remote_mcp` — no endpoint field, assert 400

---

## Phase 4 — `ArtifactBridge` (3 AI-hours)

**Principle:** Tools return structured `ToolOutput` with an `artifacts` array. A post-invoke interceptor (not inside the tool) uploads artifacts to MinIO and creates `workspace_nodes` entries. Tools stay pure — they never touch storage directly.

### 4.1 — `Artifact` and `ToolOutput` types in `common`

**New file:** `apps/backend/crates/common/src/artifact.rs`

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Artifact {
    /// Filename including extension, e.g. "transcript_2026.txt".
    pub name: String,
    /// MIME type, e.g. "text/plain", "application/pdf".
    pub mime_type: String,
    /// Base64-encoded content for small files (< 1 MiB). Mutually exclusive with `source_url`.
    #[serde(default)]
    pub data: Option<String>,
    /// Pre-signed or direct URL for large files. Mutually exclusive with `data`.
    #[serde(default)]
    pub source_url: Option<String>,
    /// Domain-specific metadata (e.g. `{"duration": 184.5, "language": "en"}`).
    #[serde(default)]
    pub metadata: Value,
}

/// Canonical tool output envelope.
/// All `CapabilityProvider::invoke()` implementations may return this as their JSON value.
/// The `ArtifactBridge` detects `artifacts` and materialises them into the workspace.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolOutput {
    /// Human-readable summary forwarded to the LLM as the tool result.
    pub content: String,
    /// Files produced by this tool invocation. May be empty.
    #[serde(default)]
    pub artifacts: Vec<Artifact>,
    /// Any extra domain metadata. Not sent to the LLM.
    #[serde(default)]
    pub metadata: Value,
}
```

**Register in `common/src/lib.rs`:** `pub mod artifact;`

### 4.2 — `ArtifactBridge` implementation

**New file:** `apps/backend/crates/agent-core/src/bridge/artifact_bridge.rs`

```rust
//! Post-invoke artifact materialisation bridge.
//!
//! Called after every `CapabilityProvider::invoke()` that returns a `ToolOutput`
//! with non-empty `artifacts`. Uploads binaries to MinIO, creates `workspace_nodes`
//! rows, and optionally triggers CocoIndex embedding for indexable MIME types.
//!
//! # SRP contract
//! - Tools return `ToolOutput` — they never call object_store or workspace_store.
//! - `ArtifactBridge` owns the upload + workspace node creation + index trigger.
//! - `CapabilityProvider::invoke()` returns a raw `serde_json::Value`.
//!   The caller (agent runtime) calls `ArtifactBridge::process_if_artifacts` after invoke.

use common::artifact::{Artifact, ToolOutput};
use common::memory::workspace::{WorkspaceNode, WorkspaceStore};
use object_store::ObjectStore;
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{info, instrument, warn};
use ulid::Ulid;

/// MIME types that should be indexed after upload.
const INDEXABLE_MIME_PREFIXES: &[&str] = &["text/", "application/pdf", "application/json"];

pub struct ArtifactBridge {
    pool: PgPool,
    object_store: Arc<dyn ObjectStore>,
    workspace_store: Arc<dyn WorkspaceStore>,
}

impl ArtifactBridge {
    pub fn new(
        pool: PgPool,
        object_store: Arc<dyn ObjectStore>,
        workspace_store: Arc<dyn WorkspaceStore>,
    ) -> Arc<Self> {
        Arc::new(Self { pool, object_store, workspace_store })
    }

    /// If `output` contains artifacts, materialise each one. Returns the output unchanged.
    #[instrument(skip(self, output), fields(tool = tool_name, artifact_count = output.artifacts.len()))]
    pub async fn process_if_artifacts(
        &self,
        tenant_id: &str,
        user_id: Option<&str>,
        tool_name: &str,
        parent_node_id: Option<&str>,
        output: &ToolOutput,
    ) -> anyhow::Result<()> {
        if output.artifacts.is_empty() {
            return Ok(());
        }
        for artifact in &output.artifacts {
            if let Err(e) = self.materialise(tenant_id, user_id, tool_name, parent_node_id, artifact).await {
                warn!(error = %e, artifact = %artifact.name, "artifact materialisation failed — skipping");
            }
        }
        Ok(())
    }

    async fn materialise(
        &self,
        tenant_id: &str,
        user_id: Option<&str>,
        tool_name: &str,
        parent_node_id: Option<&str>,
        artifact: &Artifact,
    ) -> anyhow::Result<()> {
        let node_id = Ulid::new().to_string();
        let object_key = format!("{tenant_id}/{tool_name}/{node_id}/{}", artifact.name);

        // Upload to object store if data is present.
        if let Some(ref b64) = artifact.data {
            use base64::Engine;
            let bytes = base64::engine::general_purpose::STANDARD.decode(b64)?;
            self.object_store
                .put(&object_key.clone().into(), bytes.into())
                .await?;
        }

        // Create workspace node.
        let node = WorkspaceNode {
            id: node_id.clone(),
            tenant_id: tenant_id.to_string(),
            owner_id: user_id.unwrap_or("system").to_string(),
            parent_id: parent_node_id.map(str::to_string),
            kind: "file".to_string(),
            name: artifact.name.clone(),
            virtual_path: format!("/outputs/{tool_name}/{}", artifact.name),
            metadata: json!({
                "mime_type": artifact.mime_type,
                "tool":       tool_name,
                "source":     "tool_output",
                "object_key": object_key,
                "artifact_metadata": artifact.metadata,
            }),
            ..Default::default()
        };
        self.workspace_store.create_node(&node).await?;

        // Trigger indexing for text-like MIME types (async, best-effort).
        if is_indexable(&artifact.mime_type) {
            let pool = self.pool.clone();
            let key = object_key.clone();
            let id = node_id.clone();
            tokio::spawn(async move {
                if let Err(e) = trigger_index(pool, id, key).await {
                    warn!(error = %e, "artifact index trigger failed");
                }
            });
        }

        info!(artifact = %artifact.name, node_id = %node_id, "artifact materialised");
        Ok(())
    }
}

fn is_indexable(mime: &str) -> bool {
    INDEXABLE_MIME_PREFIXES.iter().any(|p| mime.starts_with(p))
}

async fn trigger_index(pool: PgPool, node_id: String, object_key: String) -> anyhow::Result<()> {
    // Insert a pending indexing task — picked up by the indexer background task.
    sqlx::query!(
        "INSERT INTO indexing_queue (node_id, object_key, created_at)
         VALUES ($1, $2, now())
         ON CONFLICT (node_id) DO NOTHING",
        node_id,
        object_key,
    )
    .execute(&pool)
    .await?;
    Ok(())
}
```

**New file:** `apps/backend/crates/agent-core/src/bridge/mod.rs`
```rust
pub mod artifact_bridge;
pub use artifact_bridge::ArtifactBridge;
```

**Register in `agent-core/src/lib.rs`:** `pub mod bridge;`

### 4.3 — DB migration: `indexing_queue` table

**New file:** `apps/backend/crates/common/migrations/20260507000400_indexing_queue.up.sql`

```sql
CREATE TABLE IF NOT EXISTS indexing_queue (
    id          BIGSERIAL PRIMARY KEY,
    node_id     TEXT        NOT NULL,
    object_key  TEXT        NOT NULL,
    status      TEXT        NOT NULL DEFAULT 'pending',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    processed_at TIMESTAMPTZ,
    error       TEXT,
    UNIQUE (node_id)
);
CREATE INDEX IF NOT EXISTS indexing_queue_status_idx ON indexing_queue (status, created_at);
```

**Also add** to `docker/init/02-schema.sql`.

### 4.4 — Wire `ArtifactBridge` into `AppState`

**File:** `apps/backend/crates/agent-gateway/src/state.rs`

Add field:
```rust
pub artifact_bridge: Option<Arc<ArtifactBridge>>,
```

Populate in `from_env()`: construct `ArtifactBridge::new(pool, file_store, workspace_store)` only when both Postgres and MinIO are configured.

### 4.5 — Wire `ArtifactBridge` into the agent completion route

**File:** `apps/backend/crates/agent-gateway/src/routes/agent.rs`

After each tool invocation result is returned, attempt to parse it as `ToolOutput` and call `bridge.process_if_artifacts(...)`:

```rust
if let Some(ref bridge) = state.artifact_bridge {
    if let Ok(tool_out) = serde_json::from_value::<common::artifact::ToolOutput>(raw_result.clone()) {
        let _ = bridge.process_if_artifacts(
            &tenant.tenant_id,
            tenant.user_id.as_deref(),
            &tool_name,
            thread_node_id.as_deref(),
            &tool_out,
        ).await;
    }
}
```

The LLM still receives `raw_result` unchanged — `ArtifactBridge` runs as a side-effect.

### 4.6 — Tests

- `artifact_bridge_skips_empty_artifacts` — `process_if_artifacts` with no artifacts returns Ok and writes nothing
- `artifact_bridge_creates_workspace_node` — mock `WorkspaceStore`, assert `create_node` called with correct fields
- `artifact_bridge_non_indexable_mime_no_queue_entry` — verify `trigger_index` not called for `image/png`

---

## Phase 5 — Hot-Reload Wiring at Startup (1 AI-hour)

**Problem:** `RealtimeService::subscribe_capability_spec_changes()` exists but nothing calls it at startup to feed `CapabilitySpecFactory::reload_one()`. The LISTEN/NOTIFY trigger fires correctly but the receiver is never consumed.

**File:** `apps/backend/crates/agent-gateway/src/state.rs` (or `main.rs`)

In `AppState::from_env()`, after constructing `realtime_service` and `capability_spec_factory`, spawn a background task:

```rust
if let (Some(rt), Some(factory)) = (&state.realtime_service, &state.capability_spec_factory) {
    let mut rx = rt.subscribe_capability_spec_changes().await;
    let factory = Arc::clone(factory);
    let registry = Arc::clone(&state.registry);
    tokio::spawn(async move {
        while let Some((namespace, tool_name)) = rx.recv().await {
            if let Err(e) = factory.reload_one(&registry, &namespace, &tool_name).await {
                tracing::warn!(error = %e, namespace, tool_name, "hot-reload failed");
            } else {
                tracing::info!(namespace, tool_name, "capability hot-reloaded via LISTEN/NOTIFY");
            }
        }
    });
}
```

This ensures that a `capability_specs_changed` NOTIFY (from any INSERT/UPDATE/DELETE on `capability_specs` — including the new `/register` endpoint) propagates to the in-memory registry within milliseconds.

**Tests:**
- `hot_reload_task_picks_up_notify` — use `tokio::sync::mpsc` mock, verify `reload_one` called after message received

---

## Phase 6 — `services/current-time` Example Capability (2 AI-hours)

This is the zero-core-touch validation service. It lives outside `apps/backend/` and requires **zero Rust changes** to add.

### 6.1 — Directory structure

```
services/
  current-time/
    Dockerfile
    main.py
    requirements.txt
```

### 6.2 — `services/current-time/Dockerfile`

```dockerfile
FROM python:3.12-slim
WORKDIR /app
RUN pip install --no-cache-dir fastapi uvicorn mcp httpx
COPY . .
EXPOSE 8082
CMD ["uvicorn", "main:app", "--host", "0.0.0.0", "--port", "8082"]
```

### 6.3 — `services/current-time/requirements.txt`

```
fastapi>=0.115
uvicorn>=0.32
mcp>=1.0
httpx>=0.27
```

### 6.4 — `services/current-time/main.py`

```python
"""
current-time MCP capability service.
Zero-core-touch: self-registers at startup via POST /admin/capabilities/register.
"""
import asyncio
import os
from datetime import datetime
from zoneinfo import ZoneInfo, ZoneInfoNotFoundError

import httpx
from fastapi import FastAPI
from mcp.server.fastmcp import FastMCP

mcp = FastMCP("current-time")


@mcp.tool()
async def get_current_time(timezone: str = "UTC") -> dict:
    """Returns the current ISO 8601 timestamp. Optional IANA timezone name."""
    try:
        tz = ZoneInfo(timezone)
    except ZoneInfoNotFoundError:
        return {"error": f"Unknown timezone: {timezone!r}"}
    now = datetime.now(tz)
    return {
        "content": f"Current time in {timezone}: {now.isoformat()}",
        "artifacts": [],
        "metadata": {"timezone": timezone, "timestamp": now.isoformat()},
    }


app = FastAPI(title="current-time MCP service")
mcp.mount(app)


# ── Self-registration ──────────────────────────────────────────────────────────

MANIFEST = {
    "capability_id": "media.time.current-time",
    "name": "current-time",
    "namespace": "media.time",
    "description": "Returns current server time with optional IANA timezone support.",
    "version": "1.0.0",
    "kind": "remote_mcp",
    "endpoint": os.getenv("CONUSAI_SERVICE_URL", "http://current-time:8082") + "/mcp",
    "tools": [
        {
            "name": "get_current_time",
            "description": "Returns ISO 8601 timestamp for a given IANA timezone (default UTC).",
            "input_schema": {
                "type": "object",
                "properties": {
                    "timezone": {"type": "string", "description": "IANA timezone, e.g. 'Europe/Helsinki'"}
                },
            },
        }
    ],
    "tenant_scope": os.getenv("CONUSAI_TENANT_SCOPE", "").split(",")
        if os.getenv("CONUSAI_TENANT_SCOPE") else [],
    "enabled": True,
    "tags": ["time", "utility"],
}


async def register_with_retry(max_retries: int = 10, delay: float = 3.0) -> None:
    platform_url = os.environ["CONUSAI_PLATFORM_URL"]
    token = os.environ["CONUSAI_PLATFORM_TOKEN"]
    url = f"{platform_url}/admin/capabilities/register"

    for attempt in range(1, max_retries + 1):
        try:
            async with httpx.AsyncClient(timeout=10) as client:
                resp = await client.post(
                    url,
                    json=MANIFEST,
                    headers={"Authorization": f"Bearer {token}"},
                )
                resp.raise_for_status()
                print(f"[current-time] registered: {resp.json()}")
                return
        except Exception as exc:
            print(f"[current-time] registration attempt {attempt}/{max_retries} failed: {exc}")
            if attempt < max_retries:
                await asyncio.sleep(delay)

    print("[current-time] WARNING: all registration attempts failed — service running unregistered")


@app.on_event("startup")
async def on_startup() -> None:
    asyncio.create_task(register_with_retry())
```

### 6.5 — docker-compose.yml additions

**File:** `docker-compose.yml`

Add the `current-time` service to the existing compose file (no other changes):

```yaml
  current-time:
    build:
      context: ./services/current-time
    restart: unless-stopped
    environment:
      CONUSAI_PLATFORM_URL: "http://agent-gateway:8080"
      CONUSAI_PLATFORM_TOKEN: "${PLATFORM_ADMIN_TOKEN}"
      CONUSAI_SERVICE_URL: "http://current-time:8082"
      # CONUSAI_TENANT_SCOPE: "tenant-abc,tenant-xyz"  # uncomment to scope
    ports:
      - "8082:8082"
    profiles: [full]
    depends_on:
      agent-gateway:
        condition: service_healthy
```

Also add `PLATFORM_ADMIN_TOKEN=` to `.env.example` with a comment explaining it is a super-admin JWT.

---

## Phase 7 — Eval Coverage (1 AI-hour)

**File:** `apps/backend/evals/src/` (add new eval dataset + scorer)

### 7.1 — Semantic registration eval

**New file:** `apps/backend/evals/datasets/capability_registration.jsonl`

```jsonl
{"query": "what time is it in Helsinki", "expected_capabilities": ["media.time.current-time"]}
{"query": "current timestamp UTC", "expected_capabilities": ["media.time.current-time"]}
{"query": "what day of the week is it", "expected_capabilities": ["media.time.current-time"]}
```

### 7.2 — Eval runner addition

**File:** `apps/backend/evals/src/main.rs`

Add a `capability_routing` eval case that:
1. Registers `current-time` via the new `/register` endpoint against a test AppState
2. For each query in the dataset, calls `semantic_router.select(query, None)`
3. Asserts `expected_capabilities` ⊆ returned names
4. Reports recall@K score

---

## Phase 8 — Verification & Validation (1 AI-hour)

### 8.1 — Full test suite

```bash
cargo test --workspace
```

All 59 existing tests must still pass. New tests added in Phases 1–5 must also pass.

### 8.2 — Clippy clean

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Zero warnings.

### 8.3 — End-to-end smoke test

```bash
# Start infrastructure
docker compose --profile infra up -d
make db-migrate

# Start full stack
docker compose --profile full up -d

# Wait for gateway health
until curl -sf http://localhost:8080/health; do sleep 1; done

# Get super-admin JWT
TOKEN=$(curl -s -X POST http://localhost:8080/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email":"admin@test.local","password":"dev"}' | jq -r .token)

# Verify current-time auto-registered
curl -s -H "Authorization: Bearer $TOKEN" \
  "http://localhost:8080/v1/capabilities/search?q=current+time" \
  | jq '.[0].name'
# Expected: "current-time"

# Invoke via agent completions
curl -s -X POST http://localhost:8080/v1/agent/completions \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"model":"claude-opus-4-7","messages":[{"role":"user","content":"What time is it in Helsinki?"}]}' \
  | jq '.choices[0].message.content'
# Expected: string containing ISO timestamp
```

### 8.4 — Tenant scope smoke test

```bash
# Register with explicit tenant scope
curl -s -X POST http://localhost:8080/admin/capabilities/register \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "capability_id": "test.scoped-tool",
    "name": "scoped-tool",
    "namespace": "test",
    "description": "Only visible to tenant-abc",
    "version": "1.0.0",
    "kind": "remote_mcp",
    "endpoint": "http://localhost:9999/mcp",
    "tools": [{"name": "run", "description": "run", "input_schema": {"type": "object"}}],
    "tenant_scope": ["tenant-abc"]
  }'

# Search as tenant-xyz → should NOT see scoped-tool
curl -s -H "Authorization: Bearer $TOKEN_TENANT_XYZ" \
  "http://localhost:8080/v1/capabilities/search?q=scoped+tool" \
  | jq 'length'
# Expected: 0
```

---

## Phase 9 — Release & Tag (0.5 AI-hours)

```bash
# Final lint + build
make verify
cargo build --release --bin agent-gateway

# Bump workspace version in Cargo.toml: 0.3.1 → 0.3.3
sed -i '' 's/^version = "0.3.1"/version = "0.3.3"/' Cargo.toml

# Commit
git add -A
git commit -m "feat(v0.3.3): zero-core-touch mode, self-registering capabilities

- RemoteMcpCapability + ToolKind::RemoteMcp
- POST /admin/capabilities/register for external service self-registration
- tenant_scope field on capability_specs (global or per-tenant visibility)
- ArtifactBridge: tool output files → MinIO + workspace nodes
- indexing_queue table for async text artifact indexing
- Hot-reload loop: LISTEN/NOTIFY → CapabilitySpecFactory::reload_one wired at startup
- services/current-time: Python MCP self-registering example
- docker-compose: current-time service under [full] profile
- Evals: capability_routing dataset + recall@K scorer
- Migration: tenant_scope, indexing_queue"

git tag v0.3.3
```

---

## Implementation Order & Dependencies

```
Phase 1 (RemoteMcpCapability)
    ↓
Phase 2 (tenant_scope) ← independent of Phase 1 but both needed for Phase 3
    ↓
Phase 3 (register endpoint) ← requires Phases 1 + 2
    ↓
Phase 4 (ArtifactBridge)   ← independent, can run parallel to Phase 2
Phase 5 (hot-reload wiring) ← independent, can run parallel to Phase 2
    ↓
Phase 6 (services/current-time) ← requires Phase 3
    ↓
Phase 7 (evals)   ← requires Phase 3 + 6
Phase 8 (verify)  ← requires all prior phases
Phase 9 (release) ← requires Phase 8
```

---

## File Change Summary

| File | Action | Phase |
|------|--------|-------|
| `agent-core/src/tools/manifest.rs` | Add `RemoteMcp` to `ToolKind`, `tenant_scope` to `ToolManifest` | 1, 2 |
| `agent-core/src/tools/providers/remote_mcp.rs` | **New** — `RemoteMcpCapability` | 1 |
| `agent-core/src/tools/providers/mod.rs` | Register `remote_mcp` module | 1 |
| `agent-core/src/tools/providers/capability_spec.rs` | Add `remote_mcp` strategy, `tenant_scope` row field | 1, 2 |
| `agent-core/src/tools/card.rs` | Add `is_visible_to(tenant_id)` | 2 |
| `agent-core/src/tools/semantic_router.rs` | Enforce `tenant_scope` filter in `select()` | 2 |
| `agent-core/src/bridge/mod.rs` | **New** | 4 |
| `agent-core/src/bridge/artifact_bridge.rs` | **New** — `ArtifactBridge` | 4 |
| `agent-core/src/lib.rs` | `pub mod bridge` | 4 |
| `agent-core/src/realtime/mod.rs` | No changes | — |
| `agent-gateway/src/routes/admin_capabilities.rs` | Add `register_capability` handler + request types | 3 |
| `agent-gateway/src/routes/mod.rs` | Register new route | 3 |
| `agent-gateway/src/state.rs` | Add `artifact_bridge` field, spawn hot-reload task | 4, 5 |
| `agent-gateway/src/routes/agent.rs` | Wire `ArtifactBridge` after tool invocation | 4 |
| `common/src/artifact.rs` | **New** — `Artifact` + `ToolOutput` | 4 |
| `common/src/lib.rs` | `pub mod artifact` | 4 |
| `common/migrations/20260507000300_capability_tenant_scope.up.sql` | **New** | 2 |
| `common/migrations/20260507000400_indexing_queue.up.sql` | **New** | 4 |
| `docker/init/02-schema.sql` | Add `tenant_scope`, `indexing_queue` | 2, 4 |
| `docker-compose.yml` | Add `current-time` service | 6 |
| `.env.example` | Add `PLATFORM_ADMIN_TOKEN` | 6 |
| `services/current-time/Dockerfile` | **New** | 6 |
| `services/current-time/main.py` | **New** | 6 |
| `services/current-time/requirements.txt` | **New** | 6 |
| `evals/datasets/capability_registration.jsonl` | **New** | 7 |
| `evals/src/main.rs` | Add capability routing eval | 7 |
| `Cargo.toml` | Bump version to 0.3.3 | 9 |

**Total new files:** 10  
**Modified files:** 14  
**New migrations:** 2  
**Estimated AI-hours:** 14  
**Estimated tokens:** ~90k
