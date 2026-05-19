# Tenant Storage Isolation — Migration Plan

> Reference analysis: [docs/tasks/prefix-per-tenant-task.md](tasks/prefix-per-tenant-task.md)
> Related: [docs/ops/rustfs.md](ops/rustfs.md), [docs/rustfs-plan.md](rustfs-plan.md)
>
> _Revision 5 (2026‑05‑19) — polish: explicit `AbortOnDropMultipart` RAII guard, onboarding tracing span + `tenant_onboarding_total` counter (with `warn!` on marker failure), `VirtualPath: AsRef<str>` ergonomics, audit event on backfill `creds.bucket` flip. No structural changes._
>
> _Revision 4 (2026‑05‑19) — formalizes default `Workspace` root folder at provisioning time: `DEFAULT_TENANT_ROOT_NAME` constant, `TenantOnboardingService`, system‑tenant opt‑out, root‑deletion guard, terminology split between “tenant storage root” and “tenant workspace root folder”._
>
> _Revision 3 (2026‑05‑19) — adds `WorkspaceStorage` narrow trait, explicit `StorageLayout` enum for dual‑mode, `StagedUploadFinalizer` with multipart‑abort handling, backfill progress events / metrics, minor API renames._
>
> _Revision 2 (2026‑05‑19) — typed paths, private key construction, `TenantStorageMode` enum, provisioning‑time seeding, `CapabilityExecutionContext`, dedicated `StorageError`, CI guards, effort estimates._

## 0. Current State (as of 2026‑05‑19)

The platform already implements **prefix‑per‑tenant** with **per‑tenant IAM service accounts** in a single bucket (`workspace`). Key facts gathered from the codebase:

| Concern | Implementation | File |
| --- | --- | --- |
| Bucket name | Single bucket from `S3_BUCKET` (default `workspace`) | [apps/backend/crates/agent-core/src/store/rustfs_content.rs](../apps/backend/crates/agent-core/src/store/rustfs_content.rs#L29) |
| Key scheme (workspace) | `tenants/{tenant_id}/workspaces/{virtual_path}` | [rustfs_content.rs](../apps/backend/crates/agent-core/src/store/rustfs_content.rs#L119) |
| Key scheme (uploads) | `tenants/{tenant_id}/uploads/tmp/{upload_id}/{filename}` | [presign.rs](../apps/backend/crates/agent-core/src/store/presign.rs#L122) |
| Per‑tenant IAM creds | AES‑256‑GCM encrypted in redb (`CredentialStore`) | [creds.rs](../apps/backend/crates/agent-core/src/store/creds.rs) |
| Per‑tenant S3 client | LRU cache (1024 / 5 min) keyed by `tenant_id` | [rustfs_content.rs](../apps/backend/crates/agent-core/src/store/rustfs_content.rs#L86) |
| IAM provisioning | `provision_tenant` → service account with inline policy scoped to `tenants/{id}/*` | [rustfs-admin/iam.rs](../apps/backend/crates/rustfs-admin/src/iam.rs#L29), [rustfs-admin/lib.rs](../apps/backend/crates/rustfs-admin/src/lib.rs#L278) |
| Presign | SigV4 via `object_store::signer` with per‑tenant creds | [presign.rs](../apps/backend/crates/agent-core/src/store/presign.rs) |
| Tenant context | `ResolvedTenant(TenantContext)` extension via middleware | [mw/tenant.rs](../apps/backend/crates/agent-gateway/src/mw/tenant.rs) |
| Multipart complete | Concatenates parts in memory, writes via `workspace_content` | [routes/uploads.rs](../apps/backend/crates/agent-gateway/src/routes/uploads.rs#L160) |
| Bootstrap | Declarative bucket / SSE / versioning / lifecycle / CORS | [rustfs-admin/bootstrap.rs](../apps/backend/crates/rustfs-admin/src/bootstrap.rs) |

### Identified gaps

1. **Path strings are hand‑constructed in 5+ places** (`tenants/{id}/...`). Any future caller can forget the prefix.
2. **`tenant_creds` is duplicated** between `uploads.rs` and the content store; both also re‑read `S3_ENDPOINT` / `S3_BUCKET` env vars per call.
3. **Multipart `complete` loads the whole file into RAM** (`Vec<u8>` then `String::from_utf8_lossy`) — corrupts binary files and OOMs on large uploads.
4. **No new‑tenant onboarding flow** triggers `provision_tenant` + writes a default root folder; the UI shows "No folders yet" on first login (the originally reported bug).
5. **No negative tests** that exercise cross‑tenant access (presigned URL replay across tenants, direct S3 ListObjects, etc.).
6. **No per‑object tenant tag** for audit / forensic recovery.
7. **`build_root_store()` fallback** is silently used when per‑tenant creds fail to load, masking IAM provisioning errors and breaking the isolation guarantee.
8. **No virtual‑path validation** — `..`, absolute paths, control characters, NUL bytes are accepted today.

---

## Phase 1 — Harden the current prefix model

**Goal:** make today's design correct, observable, and bypass‑proof. No data migration. Delivers ~85–90 % of the isolation value at near‑zero risk.

**Estimated effort:** 8–12 hours core + 3–4 hours polish (metrics, tracing, audit, proptest).

### 1.1 Introduce `TenantStorage` — single source of truth (private key construction)

Create [`apps/backend/crates/agent-core/src/store/tenant_storage.rs`](../apps/backend/crates/agent-core/src/store/tenant_storage.rs). **All key construction is private**; callers can never obtain a raw `tenants/...` string.

```rust
use object_store::{ObjectStore, ObjectMeta, path::Path as ObjectPath, PutResult, GetResult};
use std::{sync::Arc, time::Duration};

/// Validated, normalized virtual path inside a tenant's logical workspace.
/// Constructed only via `VirtualPath::parse`, which rejects `..`, absolute
/// paths, NUL/control chars, leading slashes, empty segments, > 1024 bytes.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VirtualPath(String);

impl VirtualPath {
    pub fn parse(input: &str) -> Result<Self, StorageError> { /* ... */ }
    pub fn as_str(&self) -> &str { &self.0 }
}

impl AsRef<str> for VirtualPath {
    fn as_ref(&self) -> &str { &self.0 }
}

impl std::fmt::Display for VirtualPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(&self.0) }
}

#[derive(Clone, Copy, Debug)]
pub enum TenantStorageMode {
    /// Production: per-tenant IAM is mandatory; any miss returns an error.
    PerTenantIamRequired,
    /// Dev only: opt-in via `RUSTFS_DEV_FALLBACK_ROOT=on`. Emits CRITICAL metric.
    PerTenantIamWithDevFallback,
}

#[derive(Clone)]
pub struct TenantStorage {
    tenant_id: TenantId,
    client: Arc<dyn ObjectStore>,   // bound to one tenant's creds
    bucket: Arc<str>,                // shared in Phase 1, per-tenant in Phase 2
    creds: StorageCreds,             // for presign
    endpoint: Arc<str>,
    audit: Arc<dyn AuditSink>,
}

impl TenantStorage {
    // --- key construction stays PRIVATE ---
    fn workspace_path(&self, vp: &VirtualPath) -> ObjectPath { /* ... */ }
    fn upload_staging_path(&self, upload_id: &str, filename: &str) -> Result<ObjectPath, StorageError> { /* ... */ }

    // --- public API operates on validated inputs only ---
    pub async fn put_workspace_object(&self, vp: &VirtualPath, body: Bytes, content_type: &str) -> Result<PutResult, StorageError>;
    pub async fn get_workspace_object(&self, vp: &VirtualPath) -> Result<GetResult, StorageError>;
    pub async fn delete_workspace_object(&self, vp: &VirtualPath) -> Result<(), StorageError>;
    pub async fn list_workspace_objects(&self, vp_prefix: &VirtualPath) -> Result<Vec<ObjectMeta>, StorageError>;

    pub async fn presign_workspace_get(&self, vp: &VirtualPath, ttl: Duration, content_disposition: Option<&str>) -> Result<Url, StorageError>;
    pub async fn presign_workspace_put(&self, vp: &VirtualPath, ttl: Duration) -> Result<Url, StorageError>;
    pub async fn presign_staging_put(&self, upload_id: &str, filename: &str, ttl: Duration) -> Result<Url, StorageError>;

    pub async fn finalize_staged_upload(
        &self,
        upload_id: &str,
        parts: &[CompletedPart],   // client-asserted ETag + size per part
        dest: &VirtualPath,
    ) -> Result<FinalizeResult, StorageError>;
}

#[derive(thiserror::Error, Debug)]
pub enum StorageError {
    #[error("invalid virtual path: {0}")]
    InvalidPath(String),
    #[error("permission denied for tenant {0}")]
    PermissionDenied(TenantId),
    #[error("quota exceeded: {0}")]
    QuotaExceeded(String),
    #[error("not found")]
    NotFound,
    #[error("upstream object store error: {0}")]
    Upstream(#[from] object_store::Error),
    #[error("tenant credentials missing or invalid")]
    MissingTenantCreds,
    #[error("internal: {0}")]
    Internal(#[from] anyhow::Error),
}
```

Companion type:

```rust
pub struct TenantStorageFactory {
    mode: TenantStorageMode,
    bucket: Arc<str>,
    endpoint: Arc<str>,
    creds_store: Arc<CredentialStore>,
    client_cache: moka::sync::Cache<TenantId, Arc<dyn ObjectStore>>,
    audit: Arc<dyn AuditSink>,
}

impl TenantStorageFactory {
    pub async fn for_tenant(&self, tenant_id: &TenantId) -> Result<TenantStorage, StorageError>;
}
```

Expose `TenantStorageFactory` behind a small `StorageProvider` trait so tests can swap it with `object_store::memory::InMemory`.

**Why private keys:** returning a public `ObjectPath` from `workspace_key(...)` invites bypasses. The only way out of `TenantStorage` is through a typed op.

#### 1.1.1 Narrow capability surface — `WorkspaceStorage` trait

`TenantStorage`'s full surface (including `finalize_staged_upload`, `presign_staging_put`, raw `creds`) is too wide for agent capabilities. Introduce a focused trait that capabilities — and only capabilities — see:

```rust
/// Narrow, auditable surface for capabilities that need workspace storage.
/// Implemented by `TenantStorage`; injected into `CapabilityExecutionContext`.
#[async_trait]
pub trait WorkspaceStorage: Send + Sync {
    async fn put_object(&self, path: &VirtualPath, body: Bytes, content_type: &str)
        -> Result<PutResult, StorageError>;
    async fn get_object(&self, path: &VirtualPath) -> Result<GetResult, StorageError>;
    async fn list_objects(&self, prefix: &VirtualPath) -> Result<Vec<ObjectMeta>, StorageError>;
    async fn delete_object(&self, path: &VirtualPath) -> Result<(), StorageError>;
    async fn presign_get(&self, path: &VirtualPath, ttl: Duration, content_disposition: Option<&str>)
        -> Result<Url, StorageError>;
}

#[async_trait]
impl WorkspaceStorage for TenantStorage { /* delegates to inherent methods */ }
```

Gateway routes keep using `TenantStorage` directly (they need multipart + staging); capabilities receive `Arc<dyn WorkspaceStorage>` and cannot call multipart, staging, or root‑cred APIs even by mistake. This is the canonical SRP boundary.

#### 1.1.2 `StorageLayout` enum — explicit dual‑mode (forward‑compatible with Phase 2)

Wire layout selection in from day one so Phase 2 is a pure data change, not a control‑flow refactor:

```rust
#[derive(Clone, Debug)]
enum StorageLayout {
    /// Phase 1 + legacy tenants in Phase 2 migration window.
    LegacyPrefix { tenant_id: TenantId },     // tenants/{id}/workspaces/...
    /// Phase 2: per-tenant bucket; no tenant prefix needed.
    Modern,                                    // workspaces/...
}

impl TenantStorage {
    fn workspace_path(&self, vp: &VirtualPath) -> ObjectPath {
        match &self.layout {
            StorageLayout::LegacyPrefix { tenant_id } =>
                ObjectPath::from(format!("tenants/{tenant_id}/workspaces/{}", vp.as_str())),
            StorageLayout::Modern =>
                ObjectPath::from(format!("workspaces/{}", vp.as_str())),
        }
    }
}
```

`TenantStorageFactory::for_tenant` picks the variant based on whether `creds.bucket` is set. In Phase 1, every tenant gets `LegacyPrefix`. In Phase 2, new + migrated tenants get `Modern`.

### 1.2 Add path normalization & a `VirtualPath` newtype

`VirtualPath::parse` rejects:
- absolute paths (`/...`), Windows drive paths
- `..` segments (post‑normalization)
- empty segments / trailing `/` ambiguity
- control chars, NUL bytes
- length > 1024 bytes
- non‑UTF‑8

Add a proptest in `tenant_storage::tests` that generates random byte strings and asserts no parsed `VirtualPath` can ever escape the tenant prefix when concatenated.

### 1.3 Refactor existing call sites onto `TenantStorage`

Replace direct path construction in:
- [routes/workspaces.rs](../apps/backend/crates/agent-gateway/src/routes/workspaces.rs#L760) (`prefix = format!("tenants/...")`)
- [routes/workspaces.rs#L807](../apps/backend/crates/agent-gateway/src/routes/workspaces.rs#L807) (`object_key = format!(...)`)
- [routes/uploads.rs](../apps/backend/crates/agent-gateway/src/routes/uploads.rs#L181) (`staging_prefix`)
- [routes/internal.rs](../apps/backend/crates/agent-gateway/src/routes/internal.rs#L107) (key parsing)
- [store/quota.rs#L48](../apps/backend/crates/agent-core/src/store/quota.rs#L48)
- [store/presign.rs#L74,L96,L122](../apps/backend/crates/agent-core/src/store/presign.rs)

Re‑implement `RustFsContentStore`, `presign_*`, and the local `tenant_creds()` helper as thin wrappers over `TenantStorage`. Delete the duplicate `tenant_creds` in `uploads.rs`.

### 1.4 Tighten the root‑cred fallback (`TenantStorageMode`)

Today [rustfs_content.rs:99](../apps/backend/crates/agent-core/src/store/rustfs_content.rs#L99) silently falls back to root creds when per‑tenant load fails — a misconfigured tenant shares the root key.

```rust
match self.mode {
    TenantStorageMode::PerTenantIamRequired => {
        metrics::counter!("tenant_storage_fallback_total", "result" => "denied").increment(1);
        return Err(StorageError::MissingTenantCreds);
    }
    TenantStorageMode::PerTenantIamWithDevFallback => {
        metrics::counter!("tenant_storage_fallback_total", "result" => "dev_fallback").increment(1);
        warn!(tenant_id = %tid, "DEV FALLBACK to root creds — never set in production");
        build_root_store()?
    }
}
```

- Production builds default to `PerTenantIamRequired`.
- Add a Prometheus alert: any non‑zero `tenant_storage_fallback_total{result="dev_fallback"}` in prod fires CRITICAL.
- `RUSTFS_DEV_FALLBACK_ROOT=on` is the only way to flip the mode in dev.

### 1.5 Fix multipart `complete` — `StagedUploadFinalizer`

Replace the in‑memory placeholder in [routes/uploads.rs#L160](../apps/backend/crates/agent-gateway/src/routes/uploads.rs#L160) with a small private coordinator inside `tenant_storage.rs` so `TenantStorage` itself stays focused on primitives:

```rust
struct StagedUploadFinalizer<'a> {
    storage: &'a TenantStorage,
    upload_id: &'a str,
    parts: &'a [CompletedPart],
    dest: &'a VirtualPath,
}

impl<'a> StagedUploadFinalizer<'a> {
    async fn run(self) -> Result<FinalizeResult, StorageError> {
        // 1. head_object every staged part; verify ETag + size against `parts`; mismatch → Conflict.
        // 2. Sum sizes; quota.check(tenant, total) BEFORE opening dest multipart.
        // 3. Open `client.put_multipart(dest_path)` (object_store 0.11+).
        // 4. For each staged part: stream `get_object` body → `MultipartUpload::put_part`.
        //    No full Vec<u8> buffering.
        // 5. On ANY error after step 3: call `multipart.abort()` to release orphaned parts.
        // 6. `multipart.complete()` → returns ETag + final size.
        // 7. Best-effort delete staging objects (audit each deletion).
        // 8. Tag final PUT with x-amz-meta-tenant-id.
    }
}

impl TenantStorage {
    pub async fn finalize_staged_upload(
        &self,
        upload_id: &str,
        parts: &[CompletedPart],
        dest: &VirtualPath,
    ) -> Result<FinalizeResult, StorageError> {
        StagedUploadFinalizer { storage: self, upload_id, parts, dest }.run().await
    }
}
```

Return `FinalizeResult { virtual_path, size_bytes, etag }`.

**Critical abort path:** if `put_part` or `complete` fails after the destination multipart is open, the in‑progress upload **must** be aborted via `MultipartUpload::abort()` — otherwise S3/RustFS retains orphaned part data (storage cost + lifecycle complexity). Use an explicit RAII guard rather than ad‑hoc `scopeguard` calls in async code:

```rust
/// RAII guard: aborts the in‑progress multipart on drop unless `.complete()` was called.
/// Safer than `scopeguard` in async code because cancellation / `?` propagation always fires Drop.
struct AbortOnDropMultipart {
    upload: Option<Box<dyn object_store::MultipartUpload>>,
    dest: ObjectPath,
}

impl AbortOnDropMultipart {
    fn new(upload: Box<dyn object_store::MultipartUpload>, dest: ObjectPath) -> Self {
        Self { upload: Some(upload), dest }
    }

    /// Consumes the guard; caller is responsible for awaiting `complete()`.
    fn take(mut self) -> Box<dyn object_store::MultipartUpload> {
        self.upload.take().expect("taken twice")
    }
}

impl Drop for AbortOnDropMultipart {
    fn drop(&mut self) {
        if let Some(mut upload) = self.upload.take() {
            let dest = self.dest.clone();
            tokio::spawn(async move {
                if let Err(err) = upload.abort().await {
                    tracing::warn!(%dest, error = %err, "failed to abort orphaned multipart upload");
                    metrics::counter!("multipart_abort_failed_total").increment(1);
                }
            });
        }
    }
}
```

`StagedUploadFinalizer::run()` wraps the freshly opened multipart in `AbortOnDropMultipart` immediately after step 3; success path calls `guard.take().complete().await?`.

Test: 256 MiB binary round‑trip preserves SHA‑256, RSS stays under (initial + 32 MiB) during finalize, **and** an injected `put_part` failure leaves zero in‑progress multipart uploads on the destination (verified via `ListMultipartUploads`).

### 1.6 Default root folder — always created at provisioning time

**Decision:** every non‑system tenant gets exactly one named root folder at provisioning time. This matches the canonical pattern used by Google Drive / Dropbox / Notion / NetDocuments and gives both humans and autonomous agents a known, stable starting point. It also fixes the originally reported “No folders yet” UX bug.

#### Two “roots” — keep the terminology straight

| Term | Meaning | Lives in |
| --- | --- | --- |
| **Tenant storage root** | The S3 prefix every tenant’s objects live under. `tenants/{id}/workspaces/` in Phase 1, `workspaces/` (inside `ws-{id}`) in Phase 2. | Object store, owned by `TenantStorage` / `StorageLayout` |
| **Tenant workspace root folder** | The user‑visible top folder named `"Workspace"`. A normal folder node with `parent_id = NULL`. | redb (authoritative) + optional `_meta/seeded` marker object |

The database folder node is **authoritative**. The `_meta/seeded` marker is optional and only useful for direct S3 listing consistency.

#### Canonical constant

```rust
// agent-core::store::tenant_storage
pub const DEFAULT_TENANT_ROOT_NAME: &str = "Workspace";
```

Makes intent obvious and easy to make tenant‑configurable later (store override in tenant settings).

#### `TenantOnboardingService` — single SRP entry point

```rust
// apps/backend/crates/agent-core/src/store/onboarding.rs
pub struct TenantOnboardingService {
    workspace_store: Arc<dyn WorkspaceMetadataStore>,
    storage_factory: Arc<TenantStorageFactory>,
    creds_store: Arc<CredentialStore>,
    admin: Arc<RustFsAdminClient>,
}

pub struct OnboardingOptions {
    pub kind: TenantKind,           // Normal | System
    pub root_name: Option<String>,  // None → DEFAULT_TENANT_ROOT_NAME
}

impl TenantOnboardingService {
    /// Idempotent. Safe to re-invoke after partial failure.
    #[tracing::instrument(
        name = "tenant_onboarding",
        skip(self, owner, opts),
        fields(tenant_id = %tenant_id, kind = ?opts.kind),
    )]
    pub async fn provision(&self, tenant_id: &TenantId, owner: &UserId, opts: OnboardingOptions)
        -> Result<(), OnboardingError>
    {
        metrics::counter!("tenant_onboarding_total", "kind" => opts.kind.as_str()).increment(1);

        // 1. provision IAM service account + (Phase 2) bucket
        self.admin.provision_tenant(tenant_id).await?;

        // 2. if already seeded, return early (idempotent)
        if self.workspace_store.is_tenant_seeded(tenant_id).await? { return Ok(()); }

        // 3. skip root folder for system tenants
        if matches!(opts.kind, TenantKind::System) {
            self.workspace_store.mark_tenant_seeded(tenant_id).await?;
            return Ok(());
        }

        // 4. create the workspace root folder (parent_id = NULL, protected)
        let name = opts.root_name.as_deref().unwrap_or(DEFAULT_TENANT_ROOT_NAME);
        self.workspace_store
            .create_protected_root_folder(tenant_id, owner, name)
            .await?;

        // 5. (optional) write _meta/seeded marker via TenantStorage — log on failure, don't abort
        let storage = self.storage_factory.for_tenant(tenant_id).await?;
        if let Err(err) = storage.write_seeded_marker().await {
            tracing::warn!(error = %err, "seeded marker write failed; DB record is authoritative");
            metrics::counter!("tenant_onboarding_marker_failed_total").increment(1);
        }

        // 6. flip tenant_seeded flag atomically
        self.workspace_store.mark_tenant_seeded(tenant_id).await
    }
}
```

#### Root folder protection

The protected root folder is **a normal folder node** with one extra invariant: `is_protected_root = true`. Enforced at the `WorkspaceMetadataStore` boundary:

- `delete_node(id)` → `Err(NotAllowed)` if `is_protected_root`.
- `move_node(id, _)` → `Err(NotAllowed)` if `is_protected_root`.
- `rename_node(id, _)` → allowed (product may want renames later); requires `tenant:admin` role.
- Admin override: `DELETE /v1/admin/tenants/:id` cascades through and is allowed (used during tenant deletion).

#### Capability access helper

Expose root resolution through the narrow trait so capabilities never hardcode the name:

```rust
#[async_trait]
pub trait WorkspaceStorage: Send + Sync {
    // … existing methods …
    /// Returns the tenant's root workspace folder as a VirtualPath
    /// (`Workspace/` by default). Cheap, cached.
    async fn root_path(&self) -> Result<VirtualPath, StorageError>;
}
```

Agents can then `ctx.workspace.root_path().await?` and write artifacts under it without ever knowing the literal name.

#### Runtime safety net (for tenants that pre‑date this change)

In [routes/workspaces.rs::tree](../apps/backend/crates/agent-gateway/src/routes/workspaces.rs#L219): when `parent_id.is_none()` AND result is empty AND `!tenant_seeded`, single‑flight invoke `TenantOnboardingService::provision(tenant_id, current_user, OnboardingOptions::default())`. Single‑flight is enforced via a per‑tenant `tokio::sync::Mutex` keyed in a global `DashMap<TenantId, Arc<Mutex<()>>>`.

#### When **not** to create a root

Only for narrow opt‑outs, via `TenantKind::System`:

- Platform admin tenant.
- Audit‑only / observability tenants that own no user‑visible workspace.
- Test fixtures that explicitly want an empty state.

The default is always to create.

### 1.7 Negative isolation tests

Add `apps/backend/crates/agent-gateway/tests/tenant_isolation.rs` (runs against the real RustFS docker container in CI):

1. Provision tenants `A` and `B`; each gets per‑tenant IAM creds.
2. `A` PUTs a workspace object `secret.md`.
3. `B`'s presigned GET signed against `A`'s key → expect `403`.
4. `B`'s creds, direct `ListObjectsV2 prefix=tenants/A/` → expect `403`.
5. `B`'s creds, direct `GetObject` of `A`'s key → expect `403`.
6. Multipart: `A` initiates, `B` calls `finalize_staged_upload` with `A`'s `upload_id` → expect `404` (cannot list `A`'s staging prefix with `B`'s creds).
7. Path‑traversal: `A` posts `vp = "../B/secret.md"` → expect `StorageError::InvalidPath` (validated at `VirtualPath::parse`).

### 1.8 Audit, tracing, metadata

- Every PUT carries `x-amz-meta-tenant-id: {tenant_id}`.
- Every storage op opens a tracing span: `storage.op` with fields `tenant_id`, `actor`, `op`, `bytes`, `etag`, `result`.
- `AuditStore` records `{tenant_id, actor, op, virtual_path, bytes, etag, result, ts}` for every mutating op.
- Prometheus counters labeled by **plan tier** (not tenant id — cardinality): `storage_ops_total{op, result, tier}`.

### 1.9 CI guard against re‑introducing hand‑rolled keys

Add a justfile target `just lint-tenant-paths` invoked in CI:

```sh
! grep -rnE 'tenants/\{|format!\("tenants/' \
    --include='*.rs' \
    --exclude-dir=target \
    apps/backend/crates \
  | grep -v 'apps/backend/crates/agent-core/src/store/tenant_storage.rs' \
  || (echo "Forbidden tenant path literal outside tenant_storage.rs"; exit 1)
```

Optional follow‑up: a custom `dylint`/`clippy` rule of equivalent semantics.

### Phase 1 acceptance

- [ ] `cargo test -p agent-gateway --test tenant_isolation` passes (all 7 cases).
- [ ] `just lint-tenant-paths` green — zero `tenants/{...}` literals outside `tenant_storage.rs`.
- [ ] 256 MiB binary multipart round‑trip preserves SHA‑256; RSS delta < 32 MiB.
- [ ] New tenant sees `Workspace/` folder immediately after first login (web E2E).
- [ ] `TenantOnboardingService::provision` is idempotent (re‑run leaves DB unchanged); system tenants skip root creation; protected root folder rejects `delete_node` / `move_node` but accepts admin‑role `rename_node`.
- [ ] In a release build with `RUSTFS_PER_TENANT_IAM=on` and no creds, `for_tenant` returns `StorageError::MissingTenantCreds`; in dev mode requires explicit `RUSTFS_DEV_FALLBACK_ROOT=on` + bumps `tenant_storage_fallback_total`.
- [ ] proptest for `VirtualPath::parse` passes (10k cases) and never yields an escaping path.
- [ ] `WorkspaceStorage` trait compiled in; capabilities crate (or capability modules) hold only `Arc<dyn WorkspaceStorage>`, never `Arc<TenantStorage>`.
- [ ] Injected `put_part` failure during finalize leaves zero entries in `ListMultipartUploads` for the destination bucket.

---

## Phase 2 — Bucket‑per‑tenant migration

**Goal:** move from prefix‑per‑tenant to true **bucket‑per‑tenant** per [prefix-per-tenant-task.md](tasks/prefix-per-tenant-task.md). Because Phase 1 funneled every caller through `TenantStorage`, this is a localized refactor.

**Estimated effort:** 14–20 hours (the backfill job is the heavy piece).

### 2.1 Extend tenant provisioning

Edit [rustfs-admin/iam.rs::provision_tenant](../apps/backend/crates/rustfs-admin/src/iam.rs#L29):

1. Compute `bucket_name = sanitize_bucket_name(format!("ws-{tenant_id}"))`. Helper enforces S3 naming (3–63 chars, lowercase `[a-z0-9-]`, no leading/trailing `-`, not IP‑shaped, no `..`).
2. `client.ensure_bucket_named(&bucket_name)` — new helper on `RustFsAdminClient`.
3. Apply per‑bucket policies on the new bucket: versioning, lifecycle (`uploads/tmp/`, `exports/` — no `tenants/` prefix), encryption, CORS (if browsers PUT directly).
4. Create the service account with inline policy bound to `arn:aws:s3:::ws-{tenant_id}` + `arn:aws:s3:::ws-{tenant_id}/*` — no `s3:prefix` condition (smaller blast radius for policy bugs).
5. Persist `bucket_name` in `CredentialStore` (`StorageCreds { access_key, secret_key, created_at, bucket: Option<String> }`). `None` ⇒ legacy shared `workspace`.
6. Seed the bucket with `_meta/seeded` + the `Workspace/` folder marker (single onboarding code path — already added in 1.6).

### 2.2 `TenantStorage` becomes bucket‑aware (via `StorageLayout`)

The `StorageLayout` enum introduced in Phase 1.1.2 makes this a **data change, not a code change**. `TenantStorageFactory::for_tenant` simply selects:

```rust
let layout = match creds.bucket {
    Some(b) => StorageLayout::Modern,            // bucket = b, key = workspaces/...
    None    => StorageLayout::LegacyPrefix {     // bucket = "workspace", key = tenants/{id}/...
        tenant_id: tenant_id.clone(),
    },
};
```

No `match` lives in route handlers; the public surface of `TenantStorage` / `WorkspaceStorage` is unchanged.

### 2.3 Dual‑mode reader for the migration window

Driven entirely by `StorageLayout` selection in `for_tenant` (above). New tenants land directly on `Modern`; legacy tenants stay on `LegacyPrefix` until the backfill flips their `creds.bucket` field. Every test that exercises one layout has a sibling test for the other (parameterized via `rstest`).

### 2.4 Backfill job

Add `apps/backend/crates/jobs/src/jobs/tenant_bucket_migration.rs` integrated with the existing job runner:

1. Enumerate tenants from identity store.
2. For each tenant where `creds.bucket.is_none()`:
   1. `provision_tenant` to create the per‑tenant bucket + creds.
   2. Copy `s3://workspace/tenants/{id}/...` → `s3://ws-{id}/...` via `ObjectStore::copy` between paired clients; concurrency 16; checksum compare per object (Content‑MD5 / SHA‑256 metadata).
   3. Verify object count + total bytes; record per‑tenant migration state in redb (`pending|copying|verified|switched|cleaned`).
   4. Flip `creds.bucket = Some("ws-{id}")` (atomic redb txn). The next request uses the new bucket via 2.3. **Emit a structured `AuditStore` event** `{op: "tenant_bucket_switched", tenant_id, from: "workspace", to: "ws-{id}", objects, bytes, ts}` in the same transaction — gives a compliance‑grade timeline of every per‑tenant migration.
   5. 7‑day grace window with read mirroring → then delete the legacy `tenants/{id}/` prefix from `workspace`.
3. Idempotent + resumable — safe to re‑run; supports `--tenant <id>` for canary.

Operator gate: `just migrate-tenant-buckets [--tenant <id>] [--dry-run]`.

**Backfill observability (this is the highest‑risk operational piece):**

- Emit structured progress events on the existing task/SSE bus: `{task_id, tenant_id, phase, objects_copied, bytes_copied, objects_total, bytes_total, elapsed_ms}` every 1 s or every 100 objects, whichever is sooner.
- Prometheus metrics:
  - `tenant_bucket_migration_duration_seconds{tenant_class, result}` (histogram, labeled by **plan tier**, not tenant id)
  - `tenant_bucket_migration_objects_total{tenant_class, result}`
  - `tenant_bucket_migration_bytes_total{tenant_class, result}`
  - `tenant_bucket_migration_checksum_mismatch_total{tenant_class}` — any non‑zero value fires CRITICAL and pauses the job.
- Structured logs on every checksum mismatch with key + expected/actual digests (digests are safe to log; object bodies are not).

### 2.5 Remove the shared‑bucket fallback

Once all live tenants are switched and post‑grace:
- Drop the legacy branch in `TenantStorageFactory`.
- Drop the legacy key scheme from `TenantStorage`.
- Keep `workspace` only as a *system* bucket (e.g., `exports/global/`).

### 2.6 IAM / lifecycle simplification

- Bucket policy: simple `Resource: arn:aws:s3:::ws-{id}[, /*]` — no prefix conditions.
- Lifecycle rules are per‑bucket and prefix‑agnostic (`uploads/tmp/`, `exports/`).
- `set_versioning` per‑bucket.
- Per‑tenant SSE‑KMS keys are now trivial to wire (deferred to Phase 4).

### Phase 2 acceptance

- [ ] New tenants provisioned today land on dedicated buckets.
- [ ] Backfill job migrates a canary tenant end‑to‑end (checksum compare clean).
- [ ] All Phase 1 negative tests pass under bucket‑per‑tenant mode.
- [ ] `DELETE /v1/admin/tenants/:id` deletes the whole bucket in one call.
- [ ] No `creds.bucket.is_none()` branch remains in `main` after final switch‑over PR.

---

## Phase 3 — Capability / Rig integration (parallel with Phase 2)

**Goal:** make `TenantStorage` the only way agent capabilities touch object storage.

**Estimated effort:** 5–7 hours.

### 3.1 `CapabilityExecutionContext`

Extend the capability dispatch context (see [capabilities/workspace.rs](../apps/backend/crates/agent-gateway/src/capabilities/workspace.rs)) — note: holds the **narrow `WorkspaceStorage` trait**, not the full `TenantStorage`:

```rust
pub struct CapabilityExecutionContext {
    pub tenant_id: TenantId,
    pub actor: Actor,
    pub workspace: Arc<dyn WorkspaceStorage>,  // narrow trait from Phase 1.1.1
    pub quota: Arc<StorageQuota>,
    pub audit: Arc<dyn AuditSink>,
    pub cancel: CancellationToken,
}
```

The gateway constructs this once per tool invocation (`CapabilityProvider::dispatch`), wrapping the dispatched tenant's `TenantStorage` as `Arc<dyn WorkspaceStorage>`. Capabilities cannot call `finalize_staged_upload`, `presign_staging_put`, see `StorageCreds`, or instantiate an `ObjectStore` directly — the trait simply does not expose them.

**Ownership:** for now `CapabilityExecutionContext` stays in `agent-gateway`. If/when a separate agent runtime crate appears, move it (and `WorkspaceStorage`) into a shared `capabilities` crate.

### 3.2 Optional `StorageCapability` wrapper

For agents that need to manipulate their own artifacts as a first‑class capability, expose a narrow Rig tool surface (`workspace.read`, `workspace.write`, `workspace.list`) that thin‑wraps `TenantStorage`. This keeps the boundary auditable.

### 3.3 RAG / vector namespace isolation

For Qdrant / pgvector collections used by capabilities, confirm tenant filtering via collection name or required filter clause. Add a regression test analogous to Phase 1.7 — covered by [ADR 0009](adr/0009-redb-qdrant-rustfs.md).

### 3.4 Short‑lived agent credentials (stretch)

For long‑running agent runs, mint a 15‑minute scoped session token (derived from the tenant's IAM creds) and pass to subprocesses. Limits blast radius of a compromised capability.

### Phase 3 acceptance

- [ ] No capability implementation imports `object_store`, `rustfs_content`, or `TenantStorage` directly.
- [ ] `CapabilityExecutionContext::workspace: Arc<dyn WorkspaceStorage>` is the only documented path to storage from capabilities.
- [ ] A grep CI guard rejects new capability files referencing `S3_BUCKET`, `tenants/`, `format!("tenants/`, or `TenantStorage`.

---

## Phase 4 — Operational follow‑ups

- **Per‑tenant quota:** trivial — `aws s3api list-objects-v2 --bucket ws-{id} --no-paginate --max-items 0` or RustFS bucket metrics.
- **Per‑tenant SSE‑KMS:** wire at bucket creation in `provision_tenant`.
- **Per‑tenant deletion:** `DELETE /ws-{id}` instead of recursive prefix delete.
- **Observability:** Grafana panel "storage bytes per plan tier" (aggregate across that tier's buckets).
- **Data export / compliance:** per‑tenant bucket → customer‑owned bucket via S3 replication is one config away.

---

## Risks & mitigations

| Risk | Mitigation |
| --- | --- |
| RustFS may not scale to 10k+ buckets | Benchmark before mass migration; if hit, Phase 1‑only is a perfectly acceptable long‑term position. |
| Migration data loss | Per‑object checksum compare; 7‑day shared‑bucket retention after switch‑over; resumable job state. |
| Hand‑rolled keys re‑appear after refactor | `just lint-tenant-paths` in CI + capability grep guard (Phase 3.3). |
| Multipart finalize regressions | 256 MiB binary round‑trip + RSS assertion test (Phase 1.7). |
| Orphaned in‑progress multipart uploads | `StagedUploadFinalizer` aborts on any post‑open error; CI test asserts `ListMultipartUploads` is empty after injected failure; bucket lifecycle rule expires `AbortIncompleteMultipartUpload` after 1 day as defense‑in‑depth. |
| Silent root‑cred fallback masks IAM bugs | `TenantStorageMode::PerTenantIamRequired` is default; Prometheus alert on any fallback in prod. |
| Path traversal / unsafe inputs | `VirtualPath::parse` with proptest coverage. |
| User accidentally deletes the workspace root | `is_protected_root` flag rejects `delete_node` / `move_node` at the metadata‑store boundary; only admin tenant‑deletion cascade can remove it. |
| `tenant_storage_fallback_total` not noticed | Critical Prometheus alert with PagerDuty route. |

---

## Effort & sequencing summary

| Phase | Scope | Est. AI‑hours |
| --- | --- | --- |
| **Phase 1 core** | `TenantStorage` + `VirtualPath` + `StorageError` + `WorkspaceStorage` trait + `StorageLayout` enum + `StagedUploadFinalizer` w/ abort + refactor call sites + negative tests + provisioning‑time seeding + CI guard | **9–13** |
| Phase 1 polish | Metrics, tracing spans, audit wiring, proptest | **3–4** |
| **Phase 3 (early)** | `CapabilityExecutionContext` with narrow trait + capability grep guard | **6–8** |
| **Phase 2** | Bucket‑per‑tenant provisioning + `StorageLayout::Modern` activation + backfill job w/ progress events & metrics + IAM/lifecycle simplification | **15–21** |
| Phase 4 | Quotas, KMS, deletion, dashboards | as needed |
| **Total recommended path** | Production‑grade isolation, migrated | **33–46** |

**Recommended order:**

1. **Phase 1 + capability `CapabilityExecutionContext`** (this sprint) — highest ROI, zero migration risk; already production‑grade for prefix‑per‑tenant.
2. **Phase 2 migration** (next sprint) — only after Phase 1 negative tests are green.
3. **Phase 4** post‑migration.

Phases 1 + 3 alone eliminate ~90 % of the practical isolation risk. Phase 2 is the principled long‑term answer recommended for an agent platform handling sensitive user data.
