# RustFS Full Integration Plan

Status: living plan · Owner: platform team · Last updated: 2026-05-19 (rev 6,
review polish: `RedbHealth` startup check, `store/mod.rs` re-exports only
facades, `IndexableContent` trait, mandatory compile-fail + threat-model
gates before Phase 1, deterministic retry-queue test, refined estimate
78–95 h, long-term `aws-sigv4` lighter presigner note)

This plan turns our current "S3-compatible client pointing at RustFS" into a
**RustFS-native, multi-tenant, audit-grade object backend**. It is aggressive
and breaks backward compatibility where required. No data migrations are
provided — dev/stage are wiped, prod is bootstrapped clean.

It is organised in 8 phases. Each phase is independently shippable, gated by
a feature flag (see §4.1), and ends with concrete verification (curl + UI +
`aws-cli` against RustFS + automated tests).

> **Revision 6 changes (review polish):**
> - **`RedbHealth` startup check** in `bootstrap::storage`: opens each
>   redb file read-only, verifies header + table presence, runs a
>   round-trip AES-GCM encrypt/decrypt with `iam_enc_key` to prove the
>   key is usable, and fails boot fast with a structured error if any
>   check fails (Phase 1).
> - **`agent-core/src/store/mod.rs` is curated** to re-export only the
>   public facades and traits (`RustFsObjectStore`, `ObjectPresigner`,
>   `StorageServices`, `CredentialVault`, `StorageQuotaService`,
>   `MultipartSessionStore`, `IndexStateStore`). Concrete impls
>   (`Redb*`, `AwsSdkPresigner`) live in private submodules; handlers
>   physically cannot name them (§2.6).
> - **`IndexableContent` trait** extracted in
>   `agent-core/src/indexing/`, owned by `WorkspaceContentIndexer`. Lets
>   the indexer be promoted to a `ContentIndexingCapability` later
>   without churning the storage layer (§2.5).
> - **Mandatory gates promoted to a new §3.0 "Pre-Phase-1" section**:
>   compile-fail test for `RootAdminClient` leakage, CI `grep` check,
>   and `docs/ops/rustfs.md` threat model must land before Phase 1
>   merges.
> - **Deterministic retry-queue tests** added to the testing strategy:
>   bounded `mpsc` cap, backoff schedule (1s / 5s / 30s / 2m / 10m),
>   and drop-counter increments are asserted with a mocked clock
>   (`tokio::time::pause`) (§4.4).
> - **Etag-idempotency property test** for the indexer (§4.4).
> - **Long-term note (non-blocking)**: after Phase 3 soaks, evaluate
>   replacing `aws-sdk-s3` on the presign path with `aws-sigv4` +
>   `reqwest` to drop the heavy SDK; tracked in `docs/ops/rustfs.md`
>   under "future cleanup" (§2.1).
> - **`bon`-derived builder** for `build_storage_services` if/when its
>   argument count grows past 5 (`bon` 3 already a workspace dep)
>   (§4.8).
> - **ADR cross-reference**: trait future-proofing for Postgres swap is
>   to be documented explicitly inside
>   `docs/adr/NNNN-backend-persistent-state.md` (§4.6).
> - **Effort estimate refined to 78–95 AI-hours** (adds buffer for
>   crypto correctness, kill-9 durability tests, security review, and
>   frontend/SDK coordination). Token budget 240k–340k in / 120k–170k
>   out (§6).
>
> **Revision 5 changes (codebase alignment, fixes from arch.md cross-check):**
> - **Jobs live in the existing [`apps/backend/crates/jobs`](apps/backend/crates/jobs)
>   crate**, not duplicated under `agent-gateway/src/jobs/`. New jobs
>   (`RedbBackupJob`, `CredentialRotationJob`) are registered via the
>   existing `JobExecutor` and scheduled by `tokio-cron-scheduler` in the
>   gateway's startup wiring.
> - **`WorkspaceContentIndexer` + `IndexStateStore` stay inside
>   `agent-core/src/indexing/`** alongside `coco_indexer.rs` and
>   `embedding_service.rs`. `real_fs_watcher.rs` is retired in Phase 5
>   (replaced by the webhook). No new `workspace-indexer` crate.
> - **New `xtask` workspace member** at `apps/backend/xtask/` hosts
>   `backfill-iam`, `vault-verify`, and `vault-migrate` (added to root
>   `Cargo.toml` `[workspace.members]`).
> - **redb row encoding uses `postcard`** (workspace standard) for
>   fixed-shape records (`CredentialRecord`, `QuotaCounter`,
>   `MultipartSession`, `IndexState`). JSON is only used where a free-form
>   `serde_json::Value` is unavoidable (e.g. `WorkspaceNode.metadata`).
> - **New crates / deps enumerated** (Phase 1 step): `aws-sdk-s3`
>   (feature `real_presign`), `aes-gcm`, `secrecy`, `camino`, `subtle`,
>   `testcontainers` (dev), and the new `rustfs-admin` crate added to
>   `[workspace.members]` (§4.9).
> - **CORS allowlist includes Tauri origins** (`tauri://localhost`,
>   `https://tauri.localhost`, `null` for iOS WKWebView) configurable
>   via `RustFsConfig::cors_allowed_origins` (Phase 3, §4.1).
> - **Upload UI moves to `packages/ui/src/lib/features/upload/`** (Svelte
>   5 runes) per architectural principle #3; `apps/web` and
>   `apps/browser-shell` consume it. New SDK methods regenerated via
>   `scripts/openapi-to-types.sh` into [packages/sdk](packages/sdk).
> - **Internal webhook `POST /internal/rustfs/events` bypasses
>   `mw::identity` and `mw::tenant`** (mounted on a sub-router before
>   those layers), is `#[utoipa::path(... )]` marked with `skip`, and is
>   gated only by HMAC verification (Phase 5).
> - **Qdrant collection is the existing `content_embeddings`** (tenant
>   isolation via the existing `tenant_id` payload index), not a
>   per-tenant `workspace_{id}` collection. Matches
>   `agent-core/src/store/qdrant_vector.rs::CONTENT_COLLECTION`
>   (Phase 5).
> - **`bootstrap/` module added** at
>   `apps/backend/crates/agent-gateway/src/bootstrap/{mod,storage}.rs`
>   (file-impact map updated). `RootAdminClient` is constructable only
>   inside this module.
>
> **Revision 4 changes (review polish):**
> - Renamed `RustFsAdminClient` → `RustFsControlPlaneClient` (alias
>   `RustFsAdmin`) to make data-plane vs control-plane explicit (§2.3,
>   Phase 1).
> - Added a small **`StorageServices`** facade holding `Arc<dyn ...>` for
>   the four narrow traits, injected into handlers and the provisioner
>   (§2.5, §4.8). Traits stay narrow; only construction is centralised.
> - Promoted **`ObjectPresigner`** to a trait alongside the
>   `RustFsObjectStore` facade so the hybrid client is mockable in one
>   place. `aws-sdk-s3` is now an **optional dep gated by the
>   `real_presign` Cargo feature** (§2.1, §4.1).
> - Added explicit **`CredentialRotationJob`** (90 d default,
>   configurable grace 0–5 min) and made `cargo xtask backfill-iam`
>   idempotent + resumable via a redb marker (`backfill_iam_cursor`)
>   (Phase 2).
> - **Indexer dispatch is now fire-and-forget with a bounded internal
>   retry queue** (exponential backoff, max 5 attempts) when Qdrant is
>   briefly unavailable; HMAC verification explicitly uses `subtle`
>   /`constant_time_eq` (Phase 5).
> - **Multipart `complete` flows through `ArtifactBridge`** so workspace
>   nodes are created uniformly with single-shot uploads (Phase 6).
> - Path validation uses **`camino::Utf8PathBuf`** explicitly (§4.3).
> - Notes that `WorkspaceContentIndexer` may later be promoted to a
>   `ContentIndexingCapability` implementing `CapabilityProvider` so the
>   `SemanticCapabilityRouter` can discover it naturally — but only if it
>   grows beyond etag-idempotent upsert/delete (§2.5).
> - Added a **redb schema-evolution note** (§4.7).
> - Effort estimate tightened to **60–70 AI-hours** based on reuse of
>   existing job runner, metrics setup, and `ArtifactBridge` patterns
>   (§6).
>
> **Revision 3 changes (codebase alignment):**
> - The Rust backend has **no SQL dependency today** (Postgres only backs
>   Zitadel + Lago). Rev 2 incorrectly assumed Postgres was authoritative.
> - **Reverted to redb-authoritative** for all storage-plane metadata
>   (credentials, quota counters, multipart sessions, indexer etag state).
>   Postgres adoption for backend state is **deferred to its own ADR**
>   (`docs/adr/NNNN-backend-persistent-state.md`) and is **not blocked by
>   this plan**.
> - All persistence is hidden behind **trait abstractions**
>   (`CredentialVault`, `StorageQuotaService`, `MultipartSessionStore`,
>   `IndexStateStore`) so a future swap to Postgres is a localized change.
> - Added **redb operational guardrails**: AES-256-GCM encryption at rest
>   for secrets, `Durability::Immediate` (fsync) on rotation writes,
>   periodic snapshot of redb files into RustFS with versioning + object
>   lock (§3 Phase 2.5, §4.7).
>
> **Revision 2 changes (still in effect):**
> - Hybrid client strategy: `object_store` for data plane, `aws-sdk-s3`
>   for presign + typed control plane, custom `rustfs-admin` for
>   non-standard RustFS admin REST endpoints (§2.1).
> - Canonical naming (`RustFsObjectStore`, `ObjectPresigner`,
>   `TenantStorageProvisioner`, `StorageQuotaService`, `StorageEvent`,
>   `CredentialVault`, `StorageCreds`, `BucketNotificationHandler`,
>   `RootAdminClient`) (§2.3).
> - Single typed `RustFsConfig` / `RustFsFeatures` loaded via `figment` once
>   at startup; no scattered `std::env` reads (§4.1).
> - Root credential isolation by **newtype + module privacy**
>   (`RootAdminClient`) rather than the brittle `!Send` trick (§3 Phase 7).
> - Bootstrap reconciler produces a **drift diff report** (§3 Phase 1, §7).
> - Indexer made **etag-idempotent**; quota usage has a periodic
>   reconciler (§3 Phase 5, §3 Phase 6).

---

## 0. Current state (baseline)

Implemented:
- `RustFsContentStore` via `object_store::AmazonS3Builder`
  (`apps/backend/crates/agent-core/src/store/rustfs_content.rs`).
- Key scheme `tenants/{tenant_id}/workspaces/{virtual_path}`.
- Tenant middleware (`mw/tenant.rs`) → `TenantContext` with `storage_prefix()`.
- Per-node ACL (`owner` + `shared_with[]`) in workspace tree, 404-on-unauthorized.
- `file-storage` MCP capability: `upload_file`, `download_file`, `presigned_url`
  (token shim, not real S3 presign).
- `docker-compose.yml` runs `rustfs/rustfs:latest` + `rustfs-init` (aws-cli
  bucket bootstrap), single root credential `rustfsadmin`.
- Append-only `AuditEvent` log.

Gaps (this plan closes them):
- Single root credential; no per-tenant IAM principals/policies.
- No SSE, no versioning, no object lock, no lifecycle, no replication.
- "Presigned URL" is an opaque server-issued UUID, not a real S3 signature.
- No RustFS admin API client; bucket/policy/user config is hand-rolled in shell.
- No event-driven indexing; we poll.
- No per-tenant quota or storage-class tiering.
- No CORS / bucket policy as code.
- No backup/restore plan; no DR runbook.

---

## 1. Guiding principles

1. **Tenant = trust boundary**. Every byte read/written is scoped through
   `TenantContext`. No request can address another tenant's prefix.
2. **Least privilege at the storage layer**, not just in the gateway. RustFS
   IAM enforces tenant isolation even if the gateway is compromised.
3. **Server-side first**. Browsers/Tauri never see the root credential.
   Direct upload/download uses short-lived presigned URLs derived from
   per-tenant credentials.
4. **Audit-grade by default for paid tiers**. Versioning + object lock +
   lifecycle are enabled per plan tier, not per request.
5. **Config as code**. All buckets, policies, users, lifecycle rules,
   notification targets are declared in Rust (admin client) and reconciled
   on boot. The reconciler emits a structured drift report; never created
   by ad-hoc shell.
6. **Single source of truth for state**. **redb** is authoritative for
   storage-plane metadata (tenant credentials, quota usage, multipart
   sessions, indexer etag state). All access goes through narrow traits
   so the backing store can be swapped later (see §4.7 — Postgres adoption
   is its own ADR, not this plan).
7. **Reversible**. Each phase ships a feature flag (`RustFsFeatures::*`) so
   we can disable a new behaviour without redeploy.
8. **Idempotent everywhere**. Bootstrap reconciliation, webhook processing,
   indexer updates (by etag), and provisioning must be safe to retry.

---

## 2. Target architecture

### 2.1 Client strategy (hybrid)

| Concern | Library | Why |
|---|---|---|
| Data plane: GET/PUT/multipart/range/streaming | `object_store::AmazonS3` | Mature retry, streaming, abstraction reused across stores |
| Presigned URL signing | `aws-sdk-s3` (`PresigningConfig`) | First-class, correctly implements SigV4 query signing |
| `ListObjectVersions`, lifecycle, CORS, notification config | `aws-sdk-s3` | Typed control-plane APIs |
| RustFS-specific admin (users, policies, access keys, drift) | new `rustfs-admin` crate (`reqwest` + `serde` against `/minio/admin/v3/*`-style endpoints) | Not in any AWS SDK |

Both AWS clients are constructed with the RustFS endpoint
(`endpoint_url`, `force_path_style = true`, `region = "us-east-1"`).

The hybrid is hidden behind two narrow facades so handlers and capabilities
never name a concrete client:

- **`RustFsObjectStore`** — data-plane methods (`read`, `write`,
  `delete`, `read_range`, `list`). Implementation delegates to
  `object_store::AmazonS3` and is the single place that adds tenant
  prefix enforcement, retry, and metrics.
- **`ObjectPresigner` trait** — `presign_get` / `presign_put` /
  `presign_part`. Default impl `AwsSdkPresigner` uses `aws-sdk-s3`. A
  `NullPresigner` returns `Unimplemented` for builds without the
  `real_presign` feature.
- The `aws-sdk-s3` dependency lives behind a **Cargo feature flag
  `real_presign`** on `agent-core` so dev builds and tests that mock the
  presigner do not pull it.
- A `MemoryObjectStore` and `MemoryPresigner` impl pair is provided
  for tests (unit + capability), keeping handler tests hermetic.

**Future cleanup (non-blocking)**: after Phase 3 soaks in prod,
re-evaluate replacing `aws-sdk-s3` on the presign path with a thinner
`aws-sigv4` + `reqwest` combination to drop the heavy SDK from the
release binary. Tracked in `docs/ops/rustfs.md` under "future
cleanup"; not a Phase 8 blocker.

### 2.2 Component diagram

```
┌────────────┐   JWT/session    ┌─────────────────────────────────┐   S3 (signed)   ┌──────────┐
│  Web/Tauri │ ───────────────▶ │  agent-gateway                  │ ──────────────▶ │  RustFS  │
└────────────┘                  │  ├─ tenant mw                   │                 │  bucket: │
      ▲                         │  ├─ workspace svc               │                 │  workspace │
      │  presigned PUT/GET      │  ├─ ObjectPresigner (aws-sdk)   │                 │          │
      └─────────────────────────┤  ├─ RustFsObjectStore (object_store) │            │  IAM/    │
                                │  ├─ TenantStorageProvisioner    │ admin REST      │  policies│
                                │  ├─ StorageQuotaService         │ ──────────────▶ │          │
                                │  └─ BucketNotificationHandler   │                 │          │
                                └─────────────────────────────────┘                 └──────────┘
                                          ▲ webhook (HMAC)                ▲
                                          │                               │
                                          └──── RustFS bucket notifications
                                          │
                                          ▼ StorageEvent
                                  WorkspaceContentIndexer
```

### 2.3 Canonical names

| Concept | Type / module |
|---|---|
| Data-plane object IO | `RustFsObjectStore` (impls `object_store::ObjectStore`) |
| Per-tenant builder | `RustFsObjectStore::for_tenant(&TenantContext)` |
| Presign service | `ObjectPresigner` (uses `aws-sdk-s3`) |
| Credential persistence | `CredentialVault` trait + `RedbCredentialVault` impl (default) |
| Quota persistence | `StorageQuotaService` trait + `RedbQuotaStore` impl |
| Multipart sessions | `MultipartSessionStore` trait + `RedbMultipartStore` impl |
| Indexer etag state | `IndexStateStore` trait + `RedbIndexStateStore` impl |
| In-memory cache for decrypted creds | `StorageCreds` newtype, `moka` LRU (TTL 5 min) |
| Tenant provisioning | `TenantStorageProvisioner` |
| Control-plane client (RustFS REST) | `RustFsControlPlaneClient` (alias `RustFsAdmin`) + `RootAdminClient` newtype |
| Storage services facade (DI) | `StorageServices { object_store, presigner, vault, quota, multipart, index_state }` |
| Quota | `StorageQuotaService` + `QuotaEnforcer` |
| Webhook route | `BucketNotificationHandler` |
| Event type | `StorageEvent::{ObjectCreated, ObjectRemoved}` |
| Indexer | `WorkspaceContentIndexer` (future: `ContentIndexingCapability` impl `CapabilityProvider` if it grows beyond etag-idempotent upsert/delete) |

### 2.5 `StorageServices` facade & capability boundary

`StorageServices` is a thin, cheap-to-clone struct that bundles
`Arc<dyn ...>` references for the four narrow traits plus the object
store and presigner facades:

```rust
#[derive(Clone)]
pub struct StorageServices {
    pub object_store: Arc<RustFsObjectStore>,
    pub presigner:    Arc<dyn ObjectPresigner>,
    pub vault:        Arc<dyn CredentialVault>,
    pub quota:        Arc<dyn StorageQuotaService>,
    pub multipart:    Arc<dyn MultipartSessionStore>,
    pub index_state:  Arc<dyn IndexStateStore>,
}
```

Constructed once in `bootstrap::storage` from `RustFsConfig` +
`RustFsFeatures` and stored on `AppState`. Routes, the
`TenantStorageProvisioner`, jobs, and `ArtifactBridge` take a
`&StorageServices` (or specific `Arc<dyn ...>` slices) — no handler ever
imports a concrete redb store or AWS SDK type.

**Capability boundary** is preserved:

- `file-storage` capability remains a thin `CapabilityProvider` that
  delegates to `StorageServices::presigner` + `ArtifactBridge`. It does
  not own redb tables or AWS clients.
- Storage events / indexer output may later feed RAG-style capabilities
  without touching the storage layer.
- `SemanticCapabilityRouter` stays unaware of storage details — it only
  sees capability cards and manifests, exactly as it does for chains and
  MCP capabilities today.
- An **`IndexableContent`** trait is extracted in
  `agent-core/src/indexing/` with `fn id() -> DocId`,
  `async fn fetch_bytes() -> Result<Bytes>`,
  `fn etag() -> String`, `fn tenant() -> &TenantId`. The
  `WorkspaceContentIndexer` consumes anything implementing it, so when
  the indexer is later promoted to a `ContentIndexingCapability` it
  remains storage-agnostic.

### 2.6 `store/` module surface

`agent-core/src/store/mod.rs` re-exports **only** the public facades and
traits:

```rust
pub use object_store_facade::RustFsObjectStore;
pub use presign::ObjectPresigner;
pub use services::StorageServices;
pub use credentials::CredentialVault;
pub use meta::{StorageQuotaService, MultipartSessionStore, IndexStateStore};
```

Concrete implementations (`credentials::redb::RedbCredentialVault`,
`meta::redb::Redb*Store`, `presign::aws::AwsSdkPresigner`,
`presign::null::NullPresigner`, `presign::memory::MemoryPresigner`) live
in private submodules. Handlers physically cannot name them — the
facade and trait re-exports are the only entry points. New backends
(Postgres, in-memory test doubles) are added by adding a private
submodule and wiring it inside `bootstrap::storage::build_storage_services`.

### 2.4 Bucket layout

Single bucket `workspace`, prefix-per-tenant. Bucket-per-tenant is opt-in
for Enterprise (set on tenant record at provisioning).

```
tenants/{tenant_id}/
  workspaces/{virtual_path}          # markdown bodies + uploads
  uploads/tmp/{upload_id}/{part}     # multipart staging (lifecycle: 24h)
  exports/{ulid}.zip                 # async export jobs (lifecycle: 7d)
  audit/{yyyy}/{mm}/{dd}/{ulid}.json # mirrored audit (object-lock Enterprise)
```

---

## 3. Phases

### Phase 0 — Mandatory gates (must land before Phase 1 merges)

These are small, cheap, and protect every subsequent phase. Treated as
blockers, not nice-to-haves:

1. **Compile-fail test** in `agent-gateway/tests/compile_fail/`
   (`trybuild`) asserting that naming `RootAdminClient` outside
   `bootstrap::storage` fails to compile. Run in CI on every PR.
2. **CI grep check** (`scripts/check_root_admin_isolation.sh`): fails
   if `RootAdminClient` appears in any path other than
   `crates/agent-gateway/src/bootstrap/**` or
   `crates/rustfs-admin/src/**` (constructor + tests).
3. **Threat model** drafted in `docs/ops/rustfs.md` covering:
   compromised gateway, stolen presigned URL, AES key-rotation replay,
   cross-tenant prefix bypass, webhook HMAC replay, root credential
   leakage. Each entry lists the mitigation already in this plan.
4. **`RedbHealth` startup check** stub (Phase 1 fills in the
   credential round-trip; Phase 0 just adds the trait + boot wiring +
   structured error type).

### Phase 1 — RustFS admin client & declarative bootstrap

**Goal**: replace the `rustfs-init` shell with a Rust control-plane
client that reconciles bucket, policies, users, lifecycle, versioning,
CORS, and notifications on every gateway boot, and emits a drift diff
report.

Deliverables:
- New crate `apps/backend/crates/rustfs-admin`:
  - `RustFsControlPlaneClient::new(endpoint, root_access_key, root_secret_key)`
    (constructed only from `RootAdminClient` newtype; type alias
    `pub type RustFsAdmin = RustFsControlPlaneClient;` kept for brevity
    in handler-side code).
  - Methods (all idempotent, structured `thiserror` errors):
    `ensure_bucket`, `set_versioning`, `set_object_lock_config`,
    `put_lifecycle`, `put_cors`, `put_bucket_notification`,
    `create_user`, `attach_policy`, `put_policy`, `create_access_key`,
    `delete_access_key`, `list_keys_for_user`.
  - `reconcile(desired: DesiredStorageState) -> ReconcileReport` returns a
    **`Serialize`** diff with three machine-readable buckets
    (`added: Vec<Resource>`, `changed: Vec<ResourceDiff>`,
    `unchanged: Vec<ResourceRef>`); logged at INFO and exposed via
    `GET /admin/storage/state` (super-admin only) for UI/CI consumption.
- Bootstrap module `agent-gateway::bootstrap::storage`:
  - On startup asserts bucket `workspace` exists with versioning +
    lifecycle + CORS + notification target configured. Fully idempotent.
- Hybrid: control-plane bits already in `aws-sdk-s3`
  (lifecycle / CORS / versioning / notification config) go through it;
  RustFS-specific user/policy/access-key endpoints go through
  `rustfs-admin`.
- Remove `rustfs-init` container from `docker-compose.yml`.
- **`RedbHealth` startup check** (in `bootstrap::storage`): for each
  redb file (`iam_vault.redb`, `storage_meta.redb`, `workspace.redb`)
  opens read-only, verifies the file header and expected tables exist,
  and performs a round-trip AES-GCM encrypt+decrypt with `iam_enc_key`
  to prove the key is loaded and usable. Boot fails fast with a
  structured `HealthError { file, kind, source }` on any failure.

Verification:
- `cargo test -p rustfs-admin` against a `testcontainers`-spun RustFS.
- Boot gateway against a clean RustFS; second boot reports no drift.

Flag: `RustFsFeatures::bootstrap` (default on).

### Phase 2 — Per-tenant IAM users & credential plumb-through

**Goal**: each tenant gets its own RustFS IAM user; gateway uses that
user's credentials for all S3 operations on its behalf. Authoritative
credentials live in **redb** (encrypted at rest), behind a `CredentialVault`
trait so the store can be swapped later.

Deliverables:
- Policy template (in `rustfs-admin`):
  ```jsonc
  {
    "Version": "2012-10-17",
    "Statement": [
      { "Effect": "Allow",
        "Action": ["s3:GetObject","s3:PutObject","s3:DeleteObject",
                   "s3:AbortMultipartUpload","s3:ListMultipartUploadParts"],
        "Resource": ["arn:aws:s3:::workspace/tenants/{tenant_id}/*"] },
      { "Effect": "Allow",
        "Action": ["s3:ListBucket"],
        "Resource": ["arn:aws:s3:::workspace"],
        "Condition": { "StringLike": { "s3:prefix": ["tenants/{tenant_id}/*"] } } }
    ]
  }
  ```
- `CredentialVault` trait:
  ```rust
  #[async_trait]
  pub trait CredentialVault: Send + Sync {
      async fn put(&self, tenant: &TenantId, creds: &StorageCreds) -> Result<()>;
      async fn get(&self, tenant: &TenantId) -> Result<Option<StorageCreds>>;
      async fn rotate(&self, tenant: &TenantId, new: &StorageCreds) -> Result<()>;
      async fn delete(&self, tenant: &TenantId) -> Result<()>;
  }
  ```
  - Default impl `RedbCredentialVault` writes to a dedicated
    `iam_vault.redb` file (separate from the metadata KV) with rows
    `tenant_id -> CredentialRecord { access_key, secret_key_enc, nonce,
    key_version, created_at, rotated_at }` encoded as JSON.
  - AES-256-GCM via the `aes-gcm` crate; key from `iam_enc_key`
    (32 B, base64) in `RustFsConfig`. Nonces are 12 B random, never reused.
    `key_version` enables online re-encryption.
  - All writes use `Durability::Immediate` (fsync) so credential rotation
    survives a hard crash.
- Hot cache: in-process `moka` LRU of decrypted `StorageCreds` with TTL
  5 min, max 1024 entries. Invalidated on rotate.
- Row encoding: `CredentialRecord` is serialized with **`postcard`**
  (workspace standard for redb values). The `secret_key_enc` field is the
  AES-256-GCM ciphertext + 12 B nonce; only the cleartext secret is
  encrypted, not the whole row, so `postcard` decode never sees
  plaintext.
- Postgres alternative impl `PgCredentialVault` is **not built in this
  plan**; it is left as a future option once the backend-state ADR lands.
- `TenantStorageProvisioner::provision(tenant_id, plan_tier)`:
  1. `put_policy("tenant-{id}", rendered)`.
  2. `create_user("tenant-{id}")`.
  3. `attach_policy`.
  4. `create_access_key`.
  5. `CredentialVault::put`.
  All steps idempotent; failure rolls back partial RustFS state.
- `TenantContext::storage_credentials() -> StorageCreds` (async, vault → cache).
- `RustFsObjectStore::for_tenant(&TenantContext)` builds the per-tenant
  `AmazonS3` client.
- Backfill: `cargo xtask backfill-iam` provisions IAM for existing
  tenants in batches of 50. The `xtask` crate is a **new workspace
  member** at `apps/backend/xtask/` (added to root `Cargo.toml`).
  **Idempotent + resumable**: last processed `tenant_id` written to a
  single-row redb table `backfill_iam_cursor`; rerun continues from the
  cursor and skips tenants whose vault entry already exists.
- **`CredentialRotationJob`** lives in the existing
  [`apps/backend/crates/jobs`](apps/backend/crates/jobs) crate and is
  registered with the existing `JobExecutor`. Scheduled every
  `rotation_interval` (default 90 d) via `tokio-cron-scheduler`. For
  each due tenant: `create_access_key` → `CredentialVault::rotate` →
  wait `rotation_grace` (default 60 s, configurable 0–5 min) →
  `delete_access_key` for the previous key. Cache invalidated on rotate.

Breaking change: `RustFsObjectStore::from_env()` is removed from the
request path. Only `RootAdminClient` can construct a root-cred client
(newtype + module privacy; constructed only inside `bootstrap::storage`).

Verification:
- New tenant via REST → RustFS admin shows user + policy + key.
- Forged cross-tenant read → 403 from RustFS (not just 404 from gateway).
- `cargo test -p agent-gateway --test tenant_isolation`.
- Rotate the AES key → background job re-encrypts vault rows; old
  `key_version` count goes to 0.

Flag: `RustFsFeatures::per_tenant_iam` (off falls back to root creds, dev
only).

### Phase 3 — Real S3 presigned URLs

**Goal**: replace the UUID-token download with real `GetObject` /
`PutObject` presigned URLs signed with the tenant's IAM credentials.

Deliverables:
- `ObjectPresigner` (new module `agent-core/src/store/presign.rs`) using
  `aws-sdk-s3::presigning::PresigningConfig`:
  - `presign_get(tenant, virtual_path, ttl) -> PresignedRequest`.
  - `presign_put(tenant, virtual_path, ttl, content_type, max_bytes) -> PresignedRequest`.
  - Clamps TTL to `min(ttl, RustFsConfig::presign_ttl_max)` (default 15 min,
    hard cap 1 h).
  - Path validation (§4.3) runs **before** signing.
- HTTP endpoints (replace token-based flows):
  - `POST /v1/workspaces/{id}/presign-upload`
    `{ virtual_path, content_type, size_bytes }` →
    `{ url, method, headers, expires_at }`.
  - `GET /v1/workspaces/{id}/presign-download?virtual_path=...` →
    `{ url, expires_at }`.
  - `POST /v1/workspaces/{id}/confirm-upload`
    `{ virtual_path, etag, size }` — invokes `ArtifactBridge` to create
    the workspace node + audit event (single integration seam with the
    workspace model).
- `file-storage` capability `presigned_url` tool delegates to
  `StorageServices::presigner`. The capability manifest
  (`apps/backend/capabilities/file-storage/capability.toml`) and card
  are regenerated so the real presign tool is re-registered cleanly via
  `CapabilityFactory` / `BulkCapabilityFactory::run_bulk_load` at boot.
- HTTP error mapping for the new presign + confirm endpoints:
  - `403 forbidden` (cross-tenant or path violation) → typed envelope
    `{ code: "forbidden" }`.
  - `413 payload_too_large` (oversize / quota) → `{ code: "quota_exceeded" | "too_large" }`.
  - Surfaced consistently in `apps/web` and `apps/browser-shell` upload
    UI via existing toast + form-error patterns.
- Upload UI is shared: a new
  `packages/ui/src/lib/features/upload/` module (Svelte 5 runes
  `*.svelte.ts`) implements `createUploadFlow(sdk)` (presign → PUT →
  confirm, progress events, retry, 403/413 toast mapping). Both
  [apps/web](apps/web) and [apps/browser-shell](apps/browser-shell)
  consume it (principle #3, arch §3). New SDK methods
  (`sdk.workspaces.presignUpload/presignDownload/confirmUpload`) are
  generated from the OpenAPI spec via
  `scripts/openapi-to-types.sh` into [packages/sdk](packages/sdk).
- CORS policy on bucket allows the origins listed in
  `RustFsConfig::cors_allowed_origins` for `PUT, GET, HEAD` with headers
  `Content-Type, Authorization, x-amz-*`. Defaults (configurable):
  `http://localhost:3000`, `https://app.epifly.*`, `tauri://localhost`,
  `https://tauri.localhost`, and `null` (required for iOS WKWebView
  `file://` page origin, see `arch.md` §3 principle #5).
- Remove `download_token` table + handler; remove UUID shim.

Verification:
- DevTools shows `PUT https://rustfs.../workspace/tenants/...` 200.
- Expired URL → 403 SignatureDoesNotMatch.
- Forged cross-tenant presign attempt → 403 at RustFS.
- Playwright spec: 5 MiB upload appears in sidebar and is retrievable by
  chat.

Flag: `RustFsFeatures::real_presign`.

### Phase 4 — Encryption, versioning, object lock, lifecycle

**Goal**: durability + compliance posture matching enterprise expectations.

Deliverables:
- **SSE-S3 default**: `RustFsObjectStore::write` always sets
  `x-amz-server-side-encryption: AES256`. Enterprise + KMS:
  `aws:kms` + `x-amz-server-side-encryption-aws-kms-key-id` from tenant cfg.
- **Versioning**: enabled in Phase 1 bootstrap.
  - `GET /v1/workspaces/nodes/{id}/versions` lists versions via
    `aws-sdk-s3 ListObjectVersions`.
  - `POST /v1/workspaces/nodes/{id}/restore { version_id }` copies the
    older version over the current one (server-side copy).
- **Object lock** (Enterprise only): bucket configured with
  `ObjectLockEnabled=Enabled`. Per-node opt-in retention on
  `WorkspaceNode.retention { mode: COMPLIANCE|GOVERNANCE, until: RFC3339 }`.
  `audit/` mirror always `COMPLIANCE`, 7 years.
- **Lifecycle rules** (declared in Phase 1):
  - `uploads/tmp/*` expire after 24h.
  - `exports/*` expire after 7d.
  - Non-current versions of `workspaces/*` expire after 90d (Pro+) /
    never (Enterprise).
  - `audit/*` transition to cold class after 30d (if RustFS tiering present).
- **Replication** (optional, Enterprise): cross-region rule pointing at a
  secondary RustFS cluster. Declared per tenant.

Verification:
- `aws s3api get-bucket-versioning` → `Enabled`.
- Overwrite markdown → list versions → restore → diff.
- Delete locked object → `AccessDenied: WORM`.
- Upload to `uploads/tmp/foo`, wait, confirm GC.

Flags: `RustFsFeatures::sse`, `::versioning`, `::object_lock`.

### Phase 5 — Event-driven indexing via bucket notifications

**Goal**: kill the polling indexer; index on `s3:ObjectCreated:*` /
`s3:ObjectRemoved:*`.

Deliverables:
- RustFS notification target = webhook
  `http://agent-gateway:8080/internal/rustfs/events`
  (HMAC-SHA256 signed with `webhook_secret`).
- Route mounting: the `/internal/*` sub-router is composed **before**
  `mw::identity` and `mw::tenant` layers so it bypasses JWT extraction
  and tenant resolution. HMAC verification is the sole gate. The route
  is omitted from the OpenAPI spec (`#[utoipa::path]` not applied; or
  `SecurityAddon` skip rule) so it does not appear in Swagger or the
  generated SDK.
- `BucketNotificationHandler` route:
  - Verifies HMAC using `subtle::ConstantTimeEq` (or `constant_time_eq`)
    — no `==` on the digest.
  - Parses S3 event JSON into typed `StorageEvent`.
  - Extracts tenant id from `userIdentity.principalId` (= `tenant-{id}`).
  - Dispatches to `WorkspaceContentIndexer::on_event` as
    **fire-and-forget** (`tokio::spawn`) so the webhook returns `204`
    immediately. The handler owns a bounded in-process retry queue
    (`tokio::sync::mpsc`, cap 1024) with exponential backoff (1s, 5s,
    30s, 2m, 10m, max 5 attempts) used when Qdrant / object fetch is
    transiently unavailable. Drops past cap are logged + counted
    (`rustfs_indexer_dropped_total`); the hourly walk in Phase 6 acts as
    the safety net.
- `WorkspaceContentIndexer` lives in
  `apps/backend/crates/agent-core/src/indexing/workspace_content_indexer.rs`
  (next to `coco_indexer.rs` and `embedding_service.rs`); reuses the
  existing `EmbeddingService` trait.
  - On create/update: fetch object via `RustFsObjectStore`, parse
    (md/PDF/...), chunk, embed, upsert into the **existing single
    `content_embeddings` Qdrant collection** (per
    `agent-core/src/store/qdrant_vector.rs::CONTENT_COLLECTION`); tenant
    isolation is enforced by the existing `tenant_id` payload index. No
    per-tenant collection is created.
  - **Idempotent on etag**: skips if `(doc_id, etag)` already indexed
    via `IndexStateStore` trait (`RedbIndexStateStore` impl on
    `storage_meta.redb`, value encoded with `postcard`).
  - On delete: remove vectors by `doc_id` filter.
- Retire `agent-core/src/indexing/real_fs_watcher.rs` (the periodic
  walker) once `RustFsFeatures::notifications` is on by default.

Verification:
- Upload → Qdrant collection grows within 2 s.
- Delete → vectors gone within 2 s.
- Replay same event → no duplicate vectors.
- Bad HMAC → 401.

Flag: `RustFsFeatures::notifications`.

### Phase 6 — Quotas, tiering, multipart, large files

**Goal**: predictable cost and performance for big workloads.

Deliverables:
- `StorageQuotaService` + `QuotaEnforcer`:
  - Per-tenant **storage quota** from `PlanTier`: FREE 1 GiB, PRO 100 GiB,
    ENTERPRISE unlimited.
  - Hot counter in redb table `tenant_storage_usage` (key `tenant_id` →
    `{ bytes: u64, object_count: u64, updated_at }`), accessed via
    `StorageQuotaService` trait (so a Postgres impl can replace it later).
  - Incremented in `confirm-upload`, decremented on delete event.
  - Background reconciler every 1 h walks `ListObjectsV2` and corrects drift.
  - `QuotaEnforcer::check(tenant, additional_bytes)` called at presign-put;
    deny with HTTP 413 `code=quota_exceeded`.
- **Multipart upload** for files > 16 MiB:
  - `POST /v1/uploads/initiate { virtual_path, size, content_type }` →
    `{ upload_id, part_size }`.
  - `POST /v1/uploads/{upload_id}/parts/{n}/presign` → presigned part URL.
  - `POST /v1/uploads/{upload_id}/complete { parts: [{n, etag}] }` —
    finalises via `CompleteMultipartUpload` **and then invokes
    `ArtifactBridge::on_object_committed`** so the workspace node +
    audit event are created through the same seam as single-shot
    `confirm-upload`. No alternate node-creation path.
  - `POST /v1/uploads/{upload_id}/abort`.
  - Session state in redb table `multipart_sessions` via
    `MultipartSessionStore` trait; orphan sweeper aborts sessions older
    than 24 h.
- **Storage classes**: default `STANDARD`, exports → `STANDARD_IA`,
  audit cold → `GLACIER` (if RustFS exposes class).
- **Range reads** in `RustFsObjectStore::read_range(tenant, vp, range)`.

Verification:
- 1 GiB multipart upload completes < 30 s on LAN.
- Exceed quota → 413 `quota_exceeded`.
- Kill gateway mid-upload → orphan sweeper aborts within 24 h.

Flag: `RustFsFeatures::quotas`.

### Phase 7 — Observability, audit mirroring, security hardening

Deliverables:
- **Metrics** (Prometheus, `/metrics`):
  - `rustfs_op_latency_seconds{op,tenant}` histogram.
  - `rustfs_op_errors_total{op,code,tenant}`.
  - `rustfs_bytes_in_total{tenant}`, `rustfs_bytes_out_total{tenant}`.
  - `rustfs_storage_used_bytes{tenant}` gauge from `StorageQuotaService`.
  - `rustfs_presign_issued_total{op,tenant}`.
- **Tracing**: every `RustFsObjectStore` op `#[instrument]`-ed with
  `tenant.id`, `s3.key`, `s3.bucket`, `http.status` span fields.
- **Audit mirroring**: every mutating S3 op writes a JSON line to
  `tenants/{id}/audit/.../{ulid}.json` with object-lock retention
  (Enterprise) or 1 y lifecycle (others).
- **Security**:
  - Root credential isolation: `RootAdminClient` newtype constructable
    only inside `bootstrap::storage`; request-path code cannot name it
    (private module). Enforced by a CI `grep` check + `compile-fail`
    test (preferred over the `!Send` trick).
  - **Policy drift detection** in the reconciler: compare actual policy
    JSON against rendered template; fail boot in prod on drift
    (warn-only in dev).
  - Rotate per-tenant access keys every 90 d (background job;
    configurable 0–5 min grace before old key revoked).
  - AES-256-GCM rotation: bump `key_version`, background re-encrypt
    rows, revoke old key after all rows migrated.
  - HSTS, `Cache-Control: private, no-store` on all presign responses.
  - CSP `connect-src` includes RustFS endpoint per environment.
- **Pen-test checklist** in `docs/ops/rustfs.md`: cross-tenant prefix,
  path traversal in `virtual_path`, oversized presign TTL, replay after
  rotation, signed-URL leak window, etc.

Verification:
- `curl /metrics | grep rustfs_` shows all series.
- Rotate a tenant's key → old presigns issued before rotation become 403
  after grace window.
- Drift test: hand-edit a policy → boot fails with structured diff.

### Phase 8 — Backup, DR, multi-region

Deliverables:
- **Backup**: nightly `aws s3 sync s3://workspace s3://workspace-backup`
  to a second RustFS cluster or external S3. Runbook in
  `docs/ops/rustfs.md`.
- **PITR** leverages versioning + lifecycle (Phase 4).
- **Multi-region** (Enterprise): cross-region replication rule + a
  region-aware client `RustFsObjectStore::for_tenant_in_region`.
- **DR drill**: quarterly `make rustfs-dr-drill` spins up a fresh cluster,
  restores from backup, runs smoke tests.

Verification:
- Backup job logs success; restore reproduces `Kickoff.md` in a scratch
  cluster within RPO < 24 h and RTO < 1 h.

---

## 4. Cross-cutting changes

### 4.1 Typed configuration (single source)

Loaded once at startup via `figment` (env + `config/rustfs.toml`) and stored
in `AppState`. No scattered `std::env` reads.

```rust
#[derive(Clone, Debug, Deserialize)]
pub struct RustFsConfig {
    pub endpoint: Url,
    pub bucket: String,
    pub root_access_key: SecretString,
    pub root_secret_key: SecretString,
    pub iam_enc_key: Option<SecretString>,   // base64 32 bytes
    pub webhook_secret: Option<SecretString>,
    pub presign_ttl: Duration,                // default 15 min
    pub presign_ttl_max: Duration,            // hard cap 1 h
    pub features: RustFsFeatures,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct RustFsFeatures {
    pub bootstrap: bool,
    pub per_tenant_iam: bool,
    pub real_presign: bool,
    pub sse: bool,
    pub versioning: bool,
    pub object_lock: bool,
    pub notifications: bool,
    pub quotas: bool,
}
```

Env var mapping (`figment` profile):

| Env | Field | Default |
|-----|-------|---------|
| `RUSTFS_ENDPOINT` / `S3_ENDPOINT` | `endpoint` | `http://rustfs:9000` |
| `RUSTFS_BUCKET` / `S3_BUCKET` | `bucket` | `workspace` |
| `RUSTFS_ROOT_ACCESS_KEY` | `root_access_key` | `rustfsadmin` (dev) |
| `RUSTFS_ROOT_SECRET_KEY` | `root_secret_key` | `rustfsadmin` (dev) |
| `RUSTFS_IAM_ENC_KEY` | `iam_enc_key` | required in prod |
| `RUSTFS_WEBHOOK_SECRET` | `webhook_secret` | required in prod |
| `RUSTFS_PRESIGN_TTL_SECS` | `presign_ttl` | `900` |
| `RUSTFS_FEATURE_*` | `features.*` | see phase defaults |

### 4.2 Breaking API/UI changes

- Removed: `GET /v1/files/{token}` (UUID download shim).
- Added: `POST /v1/workspaces/{id}/presign-upload`,
  `GET /v1/workspaces/{id}/presign-download`,
  `POST /v1/workspaces/{id}/confirm-upload`,
  `GET /v1/workspaces/nodes/{id}/versions`,
  `POST /v1/workspaces/nodes/{id}/restore`,
  multipart endpoints (Phase 6),
  `GET /admin/storage/state` (super-admin drift report).
- Web/Tauri upload UI switches to direct-to-RustFS PUT; error mapping for
  403/413 added.

### 4.9 New dependencies & workspace members

Added in a single Phase 1 commit so subsequent phases compile cleanly:

- Workspace members (root `Cargo.toml [workspace.members]`):
  - `apps/backend/crates/rustfs-admin` (new)
  - `apps/backend/xtask` (new, hosts `backfill-iam`, `vault-verify`,
    `vault-migrate`)
- New `[workspace.dependencies]` entries:
  - `aws-sdk-s3 = { version = "1", default-features = false, features = ["behavior-version-latest", "rt-tokio"] }` (consumed by `agent-core` behind the `real_presign` Cargo feature)
  - `aes-gcm = "0.10"`
  - `secrecy = { version = "0.10", features = ["serde"] }` (provides `SecretString` used in `RustFsConfig`)
  - `camino = { version = "1", features = ["serde1"] }`
  - `subtle = "2"`
  - `testcontainers = "0.20"` (dev-only, used by `rustfs-admin` tests)
- `agent-core` adds a Cargo feature:
  ```toml
  [features]
  real_presign = ["dep:aws-sdk-s3"]
  ```
  Default off in dev/test; turned on in release builds where the
  presigner is needed.
- Existing deps reused as-is: `object_store` 0.11, `redb` 2,
  `postcard` 1, `moka` 0.12, `figment` 0.10, `tokio-cron-scheduler`
  0.13, `qdrant-client` 1, `tracing` / OTel 0.27, `prometheus` 0.13,
  `utoipa` 5, `proptest` 1, `wiremock` 0.6.

### 4.3 Path validation

`virtual_path` is normalized with `camino::Utf8PathBuf` (community
standard for UTF-8 paths), in a single normalizer
`store::path::normalize_virtual_path(&str) -> Result<VirtualPath>`
called from both the presigner and the workspace service. Rejected if:
- contains `..` or absolute components,
- contains NUL, CR, LF, or any byte < 0x20,
- > 1024 bytes,
- has trailing whitespace or trailing `/`,
- contains backslash on any platform.

Enforced **before** signing presigned URLs and at the workspace service
entrypoint. Covered by `proptest` property tests.

### 4.4 Testing strategy

- Unit: `rustfs-admin` against `testcontainers`-spun RustFS.
- Property: path validation with `proptest`.
- Integration: `agent-gateway` E2E suite spins up RustFS + Qdrant
  (no Postgres needed for the backend), creates two tenants, asserts
  isolation, presign roundtrip, notification → index, quota enforcement,
  drift report.
- redb durability: kill -9 mid credential rotation, restart, assert the
  rotated credentials are present (fsync semantics).
- Browser E2E: Playwright uploads a 5 MiB file via presign, asserts it
  appears in sidebar and chat retrieval finds it.
- Chaos: kill RustFS mid-upload, confirm orphan sweep aborts staged parts;
  kill gateway mid-confirm, confirm reconciler corrects quota.
- Compile-fail tests guard root-credential leakage outside `bootstrap`.
- **Etag-idempotency property test** (`proptest`) for the indexer:
  applying the same `(doc_id, etag)` event N times yields exactly one
  Qdrant upsert and zero duplicate vectors.
- **Deterministic retry-queue test**: with `tokio::time::pause`, drive
  the bounded mpsc through a synthetic failure stream and assert the
  exact backoff schedule (1s / 5s / 30s / 2m / 10m), cap behaviour at
  1024 in-flight, and `rustfs_indexer_dropped_total` increments on
  overflow.

### 4.5 Rollout

1. Land Phase 1 + 2 behind flags off in prod, on in dev/stage.
2. Soak 48 h.
3. Flip Phase 2 on in prod for new tenants only.
4. Backfill existing tenants in batches of 50 via `cargo xtask backfill-iam`
   (provision IAM, no data move — prefix already correct).
5. Land Phase 3, flip after web ships direct-upload UI.
6. Phases 4–8 follow on weekly cadence.

### 4.6 Out of scope

- Lago billing of storage bytes (separate plan).
- Cross-cloud (AWS S3, GCS) abstraction — RustFS only.
- End-to-end client-side encryption (deferred; SSE-KMS covers most needs).
- **Migrating backend persistent state to Postgres** — covered in its own
  ADR (`docs/adr/NNNN-backend-persistent-state.md`); all storage-plane
  state in this plan goes behind traits so a future swap is mechanical.
  The ADR must explicitly document the trait surface (`CredentialVault`,
  `StorageQuotaService`, `MultipartSessionStore`, `IndexStateStore`) as
  the swap boundary and list `bootstrap::storage::build_storage_services`
  as the single wiring change point.

### 4.8 `StorageServices` construction

The `StorageServices` facade (§2.5) is constructed in one place —
`bootstrap::storage::build_storage_services(cfg, features)` — which
picks the right `*Store` implementation based on `RustFsFeatures`:

- `features.real_presign = true` → `AwsSdkPresigner`; else
  `NullPresigner` (returns `Unimplemented` with a clear error).
- `features.per_tenant_iam = true` → `RedbCredentialVault`; else
  `RootFallbackVault` (dev-only, always returns the root creds).
- `features.quotas = true` → `RedbQuotaStore` + `QuotaEnforcer`; else
  `NoopQuotaService`.

This keeps the wiring DAG and feature gating in a single function and
leaves the rest of the codebase trait-only. A future Postgres impl is
added by editing this one function.

If the argument count of `build_storage_services` grows past ~5, switch
to a `bon`-derived builder (`bon` 3 is already a workspace dep) so call
sites stay readable:

```rust
let services = StorageServicesBuilder::builder()
    .config(cfg)
    .features(features)
    .clock(clock)
    .build()
    .await?;
```

### 4.7 redb operational guardrails

Because redb is authoritative for credentials, quota, multipart sessions
and indexer state, treat the redb files as first-class durable assets:

- **Files** (under `${DATA_DIR}/redb/`):
  - `iam_vault.redb` — credentials (encrypted blobs only).
  - `storage_meta.redb` — quota counters, multipart sessions, index state.
  - Existing `workspace.redb` is unchanged.
- **Durability**: writes that mutate credentials, quotas, or multipart
  sessions use `WriteTransaction::set_durability(Durability::Immediate)`
  (fsync). Read-heavy lookups use the default `Eventual`.
- **Encryption at rest**: only the secret-bearing fields are encrypted
  (AES-256-GCM with `iam_enc_key`); the file itself is **not** whole-file
  encrypted, so corrupted-row recovery remains possible.
- **Backups**:
  - Hourly `redb::Database::backup_to(path)` snapshot of each redb file.
  - Snapshot uploaded to RustFS at
    `tenants/_system/redb-backups/{file}/{yyyy}/{mm}/{dd}/{hh}.redb`,
    with bucket versioning enabled (Phase 4) and Object Lock COMPLIANCE
    30 d on this prefix (Enterprise) or 7 d lifecycle elsewhere.
  - Retention: 7 d hourly, 30 d daily.
- **Restore runbook** in `docs/ops/rustfs.md`: stop gateway → fetch latest
  snapshot from RustFS → atomic rename into `${DATA_DIR}/redb/` → start
  gateway → run `cargo xtask vault-verify`.
- **Migration path**: any `*Store` trait (vault/quota/multipart/index) can
  be re-implemented against Postgres without touching call sites; the
  ADR will define ordering.
- **Schema evolution**: each redb table value carries a leading
  `schema_version: u8`. Bump on incompatible field changes; readers
  match the version and either decode the current shape or run an
  in-place migration on first touch (writing the new shape back with
  `Durability::Immediate`). Drop-only changes are version-compatible. A
  `cargo xtask vault-migrate --to N` is added when the first migration
  is needed; until then the version byte is always `1`.

---

## 5. File/crate impact map

| Area | Path | Phase |
|------|------|-------|
| Control-plane client (REST) | `apps/backend/crates/rustfs-admin/**` (`RustFsControlPlaneClient`) — new workspace member | 1 |
| Bootstrap module | `apps/backend/crates/agent-gateway/src/bootstrap/{mod,storage}.rs` (new) — sole construction site for `RootAdminClient` and `StorageServices` | 1, 2, 7 |
| `StorageServices` facade | `apps/backend/crates/agent-core/src/store/services.rs` (new) | 2 |
| Typed config | `apps/backend/crates/agent-core/src/config/rustfs.rs` (new) | 1 |
| Credential vault | `apps/backend/crates/agent-core/src/store/credentials/{mod,redb,cache}.rs` (new, `postcard`-encoded rows) | 2 |
| Quota / multipart / index stores | `apps/backend/crates/agent-core/src/store/meta/{quota,multipart,index_state}.rs` (new, redb-backed, `postcard`-encoded) | 5, 6 |
| redb backup job | `apps/backend/crates/jobs/src/redb_backup.rs` (new, in existing `jobs` crate) | 2 |
| Credential rotation job | `apps/backend/crates/jobs/src/cred_rotation.rs` (new, in existing `jobs` crate) | 2, 7 |
| TenantContext creds | `apps/backend/crates/agent-core/src/context/tenant.rs` | 2 |
| Object store (renamed) | `apps/backend/crates/agent-core/src/store/rustfs_object_store.rs` | 2, 4, 6 |
| Presigner | `apps/backend/crates/agent-core/src/store/presign.rs` (new, gated by `real_presign` feature) | 3 |
| Tenant provisioner | `apps/backend/crates/agent-core/src/tenancy/storage_provisioner.rs` (new) | 2 |
| Quota | `apps/backend/crates/agent-core/src/quota/{service,enforcer}.rs` (new) | 6 |
| HTTP routes | `apps/backend/crates/agent-gateway/src/routes/workspaces.rs` + new `uploads.rs` | 3, 6 |
| Internal webhook (no auth/tenant mw, no OpenAPI) | `apps/backend/crates/agent-gateway/src/routes/internal.rs` (new sub-router) | 5 |
| Indexer | `apps/backend/crates/agent-core/src/indexing/workspace_content_indexer.rs` (new); retire `real_fs_watcher.rs` | 5 |
| Metrics | `apps/backend/crates/agent-gateway/src/metrics.rs` | 7 |
| Shared upload UI | `packages/ui/src/lib/features/upload/**` (new, Svelte 5 runes) | 3, 6 |
| SDK | `packages/sdk/**` regenerated via `scripts/openapi-to-types.sh` | 3, 6 |
| Web upload wrapper | `apps/web/src/routes/+page.svelte` consumes `@conusai/ui` upload feature | 3, 6 |
| Browser-shell upload wrapper | `apps/browser-shell/src/routes/+page.svelte` consumes `@conusai/ui` upload feature | 3, 6 |
| Compose | `docker-compose.yml` (drop `rustfs-init`) | 1 |
| Capability | `apps/backend/capabilities/file-storage/capability.toml` (regenerate card so router scores real presign tool) | 3 |
| xtask | `apps/backend/xtask/src/{main,backfill_iam,vault_verify,vault_migrate}.rs` — new workspace member | 2, 7 |
| Workspace manifest | root `Cargo.toml` (`[workspace.members]` + new `[workspace.dependencies]`, see §4.9) | 1 |
| Docs | `docs/ops/rustfs.md` (new), `docs/arch.md` | all |

---

## 6. Effort estimate (AI-assisted)

| Phase | Focus | Est. AI-hours |
|-------|-------|---------------|
| 0 | Mandatory gates (compile-fail, CI grep, threat model, `RedbHealth` stub) | 2–3 |
| 1 | Control-plane client + bootstrap + drift + `RedbHealth` | 7–9 |
| 2 | IAM provisioning + vault + crypto + rotation job | 10–12 |
| 3 | Presigner + routes + frontend (UI in `packages/ui`) + SDK regen | 16–20 |
| 4 | Versioning / lock / lifecycle | 7–9 |
| 5 | Notifications + indexer + retry queue + deterministic tests | 10–12 |
| 6 | Quotas + multipart (via `ArtifactBridge`) | 8–10 |
| 7 | Metrics / tracing / hardening | 6–8 |
| 8 | Backup / DR / runbook + docs | 5–6 |
| X | Cross-cutting reviews, security walkthrough, redb durability CI | 7–6 |
| **Total** | | **78–95 h** |

The band widens (was 60–70 h) to add realistic buffer for crypto
correctness review, kill -9 durability CI, cross-tenant isolation tests,
and the frontend/SDK coordination across `packages/ui`, `apps/web`, and
`apps/browser-shell`. Reuse of `crates/jobs`, `agent-gateway/src/metrics.rs`,
and the `ArtifactBridge` seam keeps it from drifting higher.

Approximate token budget: 240k–340k input, 120k–170k output.

---

## 7. Immediate next steps

1. Update root `Cargo.toml`: add `rustfs-admin` and `xtask` to
   `[workspace.members]`; add the new `[workspace.dependencies]`
   entries from §4.9 (`aws-sdk-s3`, `aes-gcm`, `secrecy`, `camino`,
   `subtle`, `testcontainers`); add the `real_presign` Cargo feature to
   `agent-core`.
2. **Land Phase 0 gates** (compile-fail test, CI grep,
   `docs/ops/rustfs.md` threat model, `RedbHealth` stub) — these are
   blockers for any Phase 1 merge.
3. Land `RustFsConfig` + `RustFsFeatures` + `figment` loader in
   `agent-core/src/config/rustfs.rs` (flags all default off in prod;
   `cors_allowed_origins` includes Tauri origins per §4.1).
4. Scaffold `apps/backend/crates/rustfs-admin` with
   `RustFsControlPlaneClient` (+ `RustFsAdmin` alias), `RootAdminClient`
   newtype, `ensure_bucket`, `set_versioning`, `reconcile` (returning a
   `Serialize` `ReconcileReport`).
5. Introduce the four narrow traits + `Redb*` impls (all rows
   `postcard`-encoded), starting with `CredentialVault` +
   `RedbCredentialVault` skeleton. Set up `iam_vault.redb` with
   `Durability::Immediate` writes and AES-256-GCM helpers.
6. Rename `RustFsContentStore` → `RustFsObjectStore` (single focused
   commit) and add the thin facade methods used by §2.5.
7. Wire the hybrid client: `object_store` primary inside
   `RustFsObjectStore`; `ObjectPresigner` trait + `AwsSdkPresigner`
   (behind `real_presign`) + `NullPresigner` + `MemoryPresigner`.
8. Add `StorageServices` facade + `build_storage_services` constructor
   in `agent-gateway/src/bootstrap/storage.rs`; wire onto `AppState` so
   handlers and the provisioner take `&StorageServices`.
9. Scaffold `apps/backend/xtask/` with `backfill-iam` (resumable via
   `backfill_iam_cursor`), `vault-verify`, `vault-migrate` subcommands.
10. Register the new jobs (`RedbBackupJob`, `CredentialRotationJob`) in
    the existing [`crates/jobs`](apps/backend/crates/jobs) crate and
    schedule them from the gateway's startup wiring.
11. Curate `agent-core/src/store/mod.rs` to re-export only facades +
    traits (§2.6); keep concrete impls in private submodules.
12. Open the separate ADR `docs/adr/NNNN-backend-persistent-state.md`
    to track whether/when backend state should move to Postgres; this
    plan does not block on it.

---

## 8. Definition of done

- All 8 phases shipped behind `RustFsFeatures` flags with defaults as above.
- E2E suite green: tenant isolation, presign roundtrip,
  notification → index, quota, versioning, object-lock (Enterprise),
  drift detection.
- Root credential not reachable from any request-handling code path
  (compile-fail test + CI grep).
- DR drill executed once successfully and documented.
- `docs/arch.md` updated to reference this plan; this file marked
  `Status: implemented` with phase completion dates.
