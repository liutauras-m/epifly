# ConusAI Platform — Multitenancy

Authoritative source: [`crates/agent-core/src/context/tenant.rs`](../crates/agent-core/src/context/tenant.rs) and [`crates/agent-gateway/src/mw/tenant.rs`](../crates/agent-gateway/src/mw/tenant.rs).

ConusAI is **multitenant from the inside out**: every storage path, Qdrant collection, log line, span, and access-control filter is keyed by `tenant_id`. The same gateway binary serves any number of tenants without code changes. The model is intentionally simple — there is no tenant table; the tenant identity is whatever the verified credential says it is.

---

## 1. Tenant identity

### `TenantContext`

```rust
// crates/agent-core/src/context/tenant.rs
pub struct TenantContext {
    pub tenant_id: String,
    pub user_id: Option<String>,   // None in dev / X-Tenant-ID mode
    pub plan: PlanTier,            // Free | Pro | Enterprise
    pub workspace_root: PathBuf,
}
```

Construction helpers:

| Method | Purpose |
|---|---|
| `tenant_root()` | `{workspace_root}/tenants/{tenant_id}` |
| `safe_path(rel)` | `safe_join` under `tenant_root()` — rejects `..` |
| `storage_prefix()` | MinIO/S3 prefix `tenants/{tenant_id}/` |
| `qdrant_collection(kind)` | `{kind}_{tenant_id}` (e.g. `threads_acme`, `workspaces_acme`, `audit_acme`) |
| `span_fields()` | Tracing pairs: `tenant_id`, `user_id?`, `plan` |

`PlanTier` provides plan-driven limits:

| Plan | `max_tokens()` | `rate_limit_rpm()` |
|---|---|---|
| Free | 4 096 | 10 |
| Pro | 16 384 | 60 |
| Enterprise | 128 000 | 600 |

### `TenantClaims` (JWT payload)

```rust
pub struct TenantClaims {
    pub sub: String,         // user_id
    pub tenant_id: String,
    pub plan: PlanTier,
    pub exp: u64,
}
```

Signed HS256. The middleware turns these claims into a `TenantContext` with `user_id = Some(sub)`.

### `effective_user_id`

For modules that must work in both prod and dev (workspace ACLs, content indexing, audit), [`common::memory::workspace::effective_user_id`](../crates/common/src/memory/workspace.rs) maps `Option<&str>` → `&str`, substituting the sentinel `"__dev__"` when the underlying `user_id` is `None`. This guarantees a single code path for owner-checks regardless of credential mode.

---

## 2. Tenant resolution middleware

[`extract_tenant`](../crates/agent-gateway/src/mw/tenant.rs) is mounted on the protected router. It branches on whether `JWT_SECRET` is set:

### Production mode — `JWT_SECRET` set

- A valid HS256 `Authorization: Bearer …` is **required**. Missing → `401 authentication required`. Invalid → `401 invalid token`.
- `X-Tenant-ID` is **ignored** (no fallback, by design — see [`docs/verify.md`](verify.md) §5.5).
- Session cookies are **not accepted** on `/v1/*` in this mode; the UI is dev-only.

### Dev mode — `JWT_SECRET` unset

Three credential sources are tried in order:

1. `X-Tenant-ID` header → `TenantContext { tenant_id, user_id: None, plan: Free, … }`.
2. `conusai_session` cookie (HMAC-signed; see [`ui/session.rs`](../crates/agent-gateway/src/ui/session.rs)) → derived `TenantContext` via `SessionUser::tenant_context()`. The cookie's `name` becomes `user_id`; tenant defaults to `"dev"` (override with `CONUSAI_UI_TENANT_ID`).
3. Nothing → `TenantContext { tenant_id: "dev", user_id: None, plan: Enterprise, … }`.

The cookie path means a browser logged in via `/login` automatically resolves to the same tenant on `/v1/*` and `/ui/*`, sharing workspaces, threads, and ACLs.

The resolved tenant is inserted as an Axum extension `ResolvedTenant(TenantContext)` and read by every protected handler:

```rust
Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
```

---

## 3. Isolation surfaces

| Surface | How it isolates | Reference |
|---|---|---|
| Filesystem workspaces | `TenantContext::safe_path(rel)` joins under `tenant_root()`; `safe_join` rejects `..` | [`common::path_safety`](../crates/common/src/path_safety.rs) |
| MinIO object keys | All keys live under `tenants/{tenant_id}/…`. Files: `tenants/{tid}/{uuid}/{filename}`. Workspace `.md`: `tenants/{tid}/workspaces/{virtual_path}` | [`routes/files.rs`](../crates/agent-gateway/src/routes/files.rs), [`memory/minio_workspace_content.rs`](../crates/agent-core/src/memory/minio_workspace_content.rs) |
| Qdrant — threads | `threads_{tenant_id}` collection | [`memory/qdrant_store.rs`](../crates/agent-core/src/memory/qdrant_store.rs) |
| Qdrant — workspaces | `workspaces_{tenant_id}` collection (text indexes on `name` + `content_text`, keyword indexes on `tenant_id`/`owner_id`/`parent_id`/`kind`/`shared_with`) | [`memory/qdrant_workspace_store.rs`](../crates/agent-core/src/memory/qdrant_workspace_store.rs) |
| Qdrant — audit | `audit_{tenant_id}` collection (ordered by `timestamp` desc) | [`memory/qdrant_audit.rs`](../crates/agent-core/src/memory/qdrant_audit.rs) |
| Qdrant — capabilities | `capabilities_{tenant_id}` (semantic capability search) | [`routes/search.rs`](../crates/agent-gateway/src/routes/search.rs) |
| Workspace ACL | Every Qdrant filter is `tenant_id == X AND (owner_id == U OR shared_with ∋ U)` (`min_should` struct form) | [`qdrant_workspace_store.rs::access_filter`](../crates/agent-core/src/memory/qdrant_workspace_store.rs) |
| Rate limiting | Per-tenant 60 s sliding window keyed by `tenant_id`, plan-driven RPM | [`mw/rate_limit.rs`](../crates/agent-gateway/src/mw/rate_limit.rs) |
| Tracing | Every span carries `tenant_id` and (when present) `user_id`, `plan` via `span_fields()` and `#[instrument(... fields(tenant_id))]` | universal |
| Audit log | `AuditEvent.tenant_id` is mandatory; the store writes/reads only that tenant's collection | [`common::audit`](../crates/common/src/audit.rs) |

The `__dev__` sentinel for owner_id only applies when no real user is known — it never spans tenants. A `dev` tenant in dev mode can access only its own data.

---

## 4. Out of scope (deferred)

- Cross-tenant sharing (workspace nodes are tenant-local; sharing accepts only `user_id` strings).
- Tenant-scoped OAuth tokens for `google-workspace`-style capabilities.
- Per-tenant capability subset (today the registry is global; tools are filtered only by what the agent receives, not by the listing).
- Quota / billing enforcement beyond rate limit RPM.
- Admin endpoints for tenant lifecycle (create / suspend / delete tenant data).

The single non-trivial cross-cutting concern that remains is **dev-mode escape**: production deployments **must** set `JWT_SECRET` so the dev fallback paths in `extract_tenant` are unreachable. `verify.md` §5.5 covers the no-fallback assertion.
