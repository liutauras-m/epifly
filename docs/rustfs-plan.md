# RustFS Full Integration Plan

Status: living plan · Owner: platform team · Last updated: 2026-05-19

This plan turns our current "S3-compatible client pointing at RustFS" into a
**RustFS-native, multi-tenant, audit-grade object backend**. It is aggressive
and breaks backward compatibility where required. No data migrations are
provided — dev/stage are wiped, prod is bootstrapped clean.

It is organised in 8 phases. Each phase is independently shippable and ends
with a concrete verification step (curl + UI + `aws-cli` against RustFS).

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
3. **Server-side first**. Browsers/Tauri never see the root credential. Direct
   upload/download uses short-lived presigned URLs derived from per-tenant
   credentials.
4. **Audit-grade by default for paid tiers**. Versioning + object lock +
   lifecycle are enabled per plan tier, not per request.
5. **Config as code**. All buckets, policies, users, lifecycle rules,
   notification targets are declared in Rust (admin client) and reconciled on
   boot, never created by ad-hoc shell.
6. **Reversible**. Each phase ships a feature flag (`RUSTFS_*`) so we can
   disable a new behaviour without redeploy.

---

## 2. Target architecture

```
┌────────────┐   JWT/session    ┌──────────────────────┐   S3 (signed)   ┌──────────┐
│  Web/Tauri │ ───────────────▶ │  agent-gateway       │ ──────────────▶ │  RustFS  │
└────────────┘                  │  ├─ tenant mw        │                 │  bucket: │
      ▲                         │  ├─ workspace svc    │                 │  workspace │
      │  presigned PUT/GET      │  ├─ rustfs admin     │ admin API       │          │
      └─────────────────────────┤  └─ presign service  │ ──────────────▶ │  IAM/    │
                                └──────────────────────┘                 │  policies│
                                          │ notifications (webhook)      └──────────┘
                                          ▼
                                  workspace indexer
```

Key choices:
- **Single bucket `workspace`**, prefix-per-tenant
  (`tenants/{tenant_id}/...`). Bucket-per-tenant is opt-in for Enterprise.
- **Per-tenant IAM user** in RustFS with policy restricting it to
  `arn:aws:s3:::workspace/tenants/{tenant_id}/*`.
- **Per-tenant STS-style ephemeral credentials** (or rotated long-lived
  credentials cached in redb) handed to the gateway request handler via
  `TenantContext::storage_credentials()`.
- **Presigned URLs** issued by the gateway using the tenant's credentials —
  the browser uploads directly to RustFS, no proxy.

Prefix layout inside the bucket:
```
tenants/{tenant_id}/
  workspaces/{virtual_path}        # markdown bodies + uploads
  uploads/tmp/{ulid}                # multipart staging (lifecycle: expire 24h)
  exports/{ulid}.zip                # async export jobs (lifecycle: expire 7d)
  audit/{yyyy}/{mm}/{dd}/{ulid}.json # mirrored audit (object-lock if Enterprise)
```

---

## 3. Phases

### Phase 1 — RustFS admin client & declarative bootstrap

**Goal**: replace `rustfs-init` shell with a Rust admin client that reconciles
bucket, policies, users, lifecycle, versioning, CORS, notifications on every
gateway boot.

Deliverables:
- New crate `apps/backend/crates/rustfs-admin`:
  - `RustFsAdminClient::new(endpoint, root_access_key, root_secret_key)`.
  - Methods: `ensure_bucket`, `set_versioning`, `set_object_lock_config`,
    `put_lifecycle`, `put_cors`, `put_bucket_notification`,
    `create_user`, `attach_policy`, `put_policy`, `create_access_key`,
    `delete_access_key`, `list_keys_for_user`.
  - Implementation uses RustFS admin REST API (S3-compatible MinIO-style
    admin endpoints documented at https://docs.rustfs.com).
- Bootstrap module `agent-gateway::bootstrap::storage`:
  - On startup, asserts bucket `workspace` exists with versioning + lifecycle
    + CORS configured. Idempotent.
- Remove `rustfs-init` container from `docker-compose.yml`.

Verification:
- `cargo test -p rustfs-admin` against a docker-compose'd RustFS.
- Boot gateway against a clean RustFS, confirm bucket+policies created.

Feature flag: `RUSTFS_BOOTSTRAP=on|off` (default on).

### Phase 2 — Per-tenant IAM users and credential plumb-through

**Goal**: each tenant gets its own RustFS IAM user; gateway uses that user's
credentials for all S3 operations on behalf of the tenant.

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
- Provisioning hook: when a tenant is created (Zitadel webhook or
  `POST /v1/tenants`), `TenantProvisioner` calls
  `rustfs-admin::provision_tenant(tenant_id)`:
  1. `put_policy("tenant-{id}", rendered)`.
  2. `create_user("tenant-{id}")`.
  3. `attach_policy`.
  4. `create_access_key` → store `(access_key, secret_key)` encrypted in
     redb under `iam/tenant/{tenant_id}`.
- New `TenantContext::storage_credentials() -> StorageCreds` returns the
  per-tenant key pair (decrypted on demand).
- `RustFsContentStore::for_tenant(&TenantContext)` builds an `AmazonS3`
  client with those creds (LRU-cached, max 1024 entries, 5 min TTL).
- Encryption: AES-256-GCM with key from `RUSTFS_IAM_ENC_KEY` env (32 bytes,
  base64). Rotation procedure documented in `docs/ops/rustfs.md`.

Breaking change: `RustFsContentStore::from_env()` only used for the
**root** path (admin client). All workspace IO must go through
`for_tenant`. Compile-time enforced by removing the generic implementation
from `WorkspaceContentStore`.

Verification:
- New tenant provisioned via REST → RustFS admin shows user + policy.
- Gateway attempts cross-tenant read → 403 from RustFS (not just 404 from
  gateway).
- `cargo test -p agent-gateway --test tenant_isolation`.

Feature flag: `RUSTFS_PER_TENANT_IAM=on|off`. Off falls back to root creds
(dev only).

### Phase 3 — Real S3 presigned URLs

**Goal**: replace UUID-token download with real `GetObject`/`PutObject`
presigned URLs signed with the tenant's IAM credentials.

Deliverables:
- `presign` module in `agent-core`:
  - `presign_get(tenant, virtual_path, ttl)` → `Url`.
  - `presign_put(tenant, virtual_path, ttl, content_type, max_bytes)` → `Url`.
  - Uses `object_store::signer::Signer` (preferred) or `aws-sigv4` crate
    directly. Returns URLs valid for `min(ttl, 1h)`; default 15 min.
- HTTP endpoints (replace token-based flows):
  - `POST /v1/workspaces/{id}/presign-upload`
    body `{ virtual_path, content_type, size_bytes }` →
    `{ url, headers, expires_at }`.
  - `GET /v1/workspaces/{id}/presign-download?virtual_path=...` →
    `{ url, expires_at }`.
- `file-storage` capability `presigned_url` tool now returns the real URL.
- Frontend `apps/web` upload flow updated:
  - Request presign → `PUT` directly to RustFS → `POST` confirm to gateway
    (records node metadata + audit event).
- CORS policy on bucket allows
  `Origin: http://localhost:3000`, `https://app.epifly.*` for
  `PUT, GET, HEAD` with headers `Content-Type, Authorization, x-amz-*`.
- Remove `download_token` table + handler; remove UUID shim.

Verification:
- Browser DevTools shows `PUT https://rustfs.../workspace/tenants/...` 200.
- `curl` with expired URL → 403 SignatureDoesNotMatch.
- Cross-tenant presign attempt (forged path) → 403 at RustFS.

Feature flag: `RUSTFS_REAL_PRESIGN=on|off`.

### Phase 4 — Encryption, versioning, object lock, lifecycle

**Goal**: durability + compliance posture matching enterprise expectations.

Deliverables:
- **SSE-S3 default**: `RustFsContentStore::write` always sets
  `x-amz-server-side-encryption: AES256`. For Enterprise tier with KMS:
  `aws:kms` + `x-amz-server-side-encryption-aws-kms-key-id` from tenant
  config.
- **Versioning**: enabled on bucket in Phase 1 bootstrap. New API
  `GET /v1/workspaces/nodes/{id}/versions` lists versions (S3
  `ListObjectVersions`); `POST .../restore` copies an older version over
  the current one.
- **Object lock** (Enterprise only): bucket configured with
  `ObjectLockEnabled=Enabled`. Per-node opt-in retention via
  `WorkspaceNode.retention { mode: COMPLIANCE|GOVERNANCE, until: RFC3339 }`.
  Mirror of `audit/` prefix uses `COMPLIANCE` + 7 years.
- **Lifecycle rules** (declared in Phase 1):
  - `uploads/tmp/*` expire after 24h.
  - `exports/*` expire after 7d.
  - Non-current versions of `workspaces/*` expire after 90d (Pro+) /
    never (Enterprise).
  - `audit/*` transition to cold storage class after 30d (if supported by
    RustFS tiering).
- **Replication** (optional, Enterprise): cross-region rule pointing at a
  secondary RustFS cluster. Declared per tenant.

Verification:
- `aws s3api get-bucket-versioning` → `Enabled`.
- Overwrite a markdown file, list versions, restore, diff.
- Attempt to delete a locked object → `AccessDenied: WORM`.
- Upload to `uploads/tmp/foo`, wait, confirm GC.

Feature flag: `RUSTFS_SSE=on|off`, `RUSTFS_VERSIONING=on|off`,
`RUSTFS_OBJECT_LOCK=on|off`.

### Phase 5 — Event-driven indexing via bucket notifications

**Goal**: kill the polling indexer; index on `s3:ObjectCreated:*` /
`s3:ObjectRemoved:*`.

Deliverables:
- RustFS notification target = webhook
  `http://agent-gateway:8080/internal/rustfs/events`
  (HMAC-signed with `RUSTFS_WEBHOOK_SECRET`).
- New route `internal_routes::rustfs_events`:
  - Verifies HMAC.
  - Parses S3 event JSON, extracts `bucket`, `key`, `eventName`, `eTag`,
    `size`, `userIdentity` (per-tenant IAM principal → tenant id).
  - Dispatches to `WorkspaceIndexer::on_object_event`.
- `WorkspaceIndexer`:
  - On create/update: fetch object, parse (markdown/PDF/...), chunk,
    embed, upsert into Qdrant collection `workspace_{tenant_id}`.
  - On delete: remove vectors by `doc_id`.
- Remove the periodic walker.

Verification:
- Upload a file, watch `Qdrant` collection grow within 2s.
- Delete the file, vectors gone within 2s.
- Replay with bad HMAC → 401.

Feature flag: `RUSTFS_NOTIFICATIONS=on|off`.

### Phase 6 — Quotas, tiering, multipart, large files

**Goal**: predictable cost and performance for big workloads.

Deliverables:
- Per-tenant **storage quota** in `PlanTier`:
  - FREE 1 GiB, PRO 100 GiB, ENTERPRISE unlimited.
  - Enforced by `QuotaService` that aggregates
    `ListObjectsV2` totals (cached 60s in redb).
  - `presign_put` denies (HTTP 413) if `used + size > quota`.
- **Multipart upload** for files > 16 MiB:
  - `POST /v1/uploads/initiate` → `uploadId`.
  - `POST /v1/uploads/{uploadId}/parts/{n}/presign` → presigned part URL.
  - `POST /v1/uploads/{uploadId}/complete` body `{ parts: [{n, etag}] }`.
  - `POST /v1/uploads/{uploadId}/abort`.
- **Storage classes** (if RustFS supports):
  - default `STANDARD`, exports → `STANDARD_IA`, audit cold → `GLACIER`.
- **Range reads** in `RustFsContentStore::read_range(tenant, vp, range)`
  for previewing large logs.

Verification:
- Upload 1 GiB file via multipart from CLI, completes < 30s on LAN.
- Exceed quota → 413 with `code=quota_exceeded`.

Feature flag: `RUSTFS_QUOTAS=on|off`.

### Phase 7 — Observability, audit mirroring, security hardening

Deliverables:
- **Metrics** (Prometheus, scraped at `/metrics`):
  - `rustfs_op_latency_seconds{op,tenant}` histogram.
  - `rustfs_op_errors_total{op,code,tenant}`.
  - `rustfs_bytes_in_total{tenant}`, `rustfs_bytes_out_total{tenant}`.
  - `rustfs_storage_used_bytes{tenant}` gauge from QuotaService.
- **Tracing**: every `RustFsContentStore` op already `#[instrument]`-ed;
  add `tenant.id`, `s3.key`, `s3.bucket`, `http.status` span fields.
- **Audit mirroring**: every mutating S3 op writes a JSON line to
  `tenants/{id}/audit/.../{ulid}.json` with object-lock retention
  (Enterprise) or 1 y lifecycle (others).
- **Security**:
  - Root credential only loaded in admin client; gateway request path
    cannot construct an `AmazonS3` with root creds (compile-time guard:
    `RootCredentials` is `!Send` outside `admin::*`).
  - Rotate per-tenant access keys every 90d (background job).
  - HSTS, `Cache-Control: private, no-store` on all presign responses.
  - CSP `connect-src` includes RustFS endpoint per environment.
- **Pen-test checklist** in `docs/ops/rustfs.md`:
  cross-tenant prefix, path traversal in `virtual_path`, oversized
  presign TTL, replay of presigned URL after rotation, etc.

Verification:
- `curl /metrics | grep rustfs_` shows all series.
- Rotate a tenant's key, old presigns issued before rotation become 403
  after key revoke (configurable: immediate vs 5 min grace).

### Phase 8 — Backup, DR, multi-region

Deliverables:
- **Backup**: nightly `aws s3 sync s3://workspace s3://workspace-backup`
  to a second RustFS cluster or external S3. Restore runbook in
  `docs/ops/rustfs.md`.
- **Point-in-time recovery** leverages versioning + lifecycle (Phase 4).
- **Multi-region** (Enterprise): cross-region replication rule + a
  region-aware client that prefers nearest endpoint
  (`RustFsContentStore::for_tenant_in_region`).
- **DR drill**: quarterly tabletop with `make rustfs-dr-drill` script
  that spins up a fresh cluster, restores from backup, runs smoke tests.

Verification:
- Backup job logs success; restore script reproduces `Kickoff.md` in a
  scratch cluster within RPO target (< 24h) and RTO target (< 1h).

---

## 4. Cross-cutting changes

### 4.1 Config surface (gateway env)

| Var | Default | Purpose |
|-----|---------|---------|
| `S3_ENDPOINT` | `http://rustfs:9000` | RustFS endpoint |
| `S3_BUCKET` | `workspace` | Primary bucket |
| `RUSTFS_ROOT_ACCESS_KEY` | `rustfsadmin` | Admin client only |
| `RUSTFS_ROOT_SECRET_KEY` | `rustfsadmin` | Admin client only |
| `RUSTFS_IAM_ENC_KEY` | (required prod) | AES-256-GCM key for per-tenant creds |
| `RUSTFS_WEBHOOK_SECRET` | (required prod) | HMAC for bucket notifications |
| `RUSTFS_BOOTSTRAP` | `on` | Run declarative bootstrap on boot |
| `RUSTFS_PER_TENANT_IAM` | `on` | Phase 2 |
| `RUSTFS_REAL_PRESIGN` | `on` | Phase 3 |
| `RUSTFS_SSE` | `on` | Phase 4 |
| `RUSTFS_VERSIONING` | `on` | Phase 4 |
| `RUSTFS_OBJECT_LOCK` | `off` | Phase 4, Enterprise |
| `RUSTFS_NOTIFICATIONS` | `on` | Phase 5 |
| `RUSTFS_QUOTAS` | `on` | Phase 6 |
| `RUSTFS_PRESIGN_TTL_SECS` | `900` | Max 3600 |

### 4.2 Breaking API/UI changes

- Removed: `GET /v1/files/{token}` (UUID download shim).
- Added: `POST /v1/workspaces/{id}/presign-upload`,
  `GET /v1/workspaces/{id}/presign-download`,
  `GET /v1/workspaces/nodes/{id}/versions`,
  `POST /v1/workspaces/nodes/{id}/restore`,
  multipart endpoints (Phase 6).
- Web/Tauri upload UI switches to direct-to-RustFS PUT; loading
  states reused; error mapping for 403/413 added.

### 4.3 Path validation

`virtual_path` is normalized with `Utf8PathBuf`, rejected if:
- contains `..` or absolute components,
- contains NUL, CR, LF, or any byte < 0x20,
- > 1024 bytes,
- has trailing whitespace or trailing `/`.

This is enforced **before** signing presigned URLs and at the workspace
service entrypoint. Tested with property tests (`proptest`).

### 4.4 Testing strategy

- Unit: `rustfs-admin` against a containerised RustFS (`testcontainers`).
- Integration: `agent-gateway` E2E suite spins up RustFS + Postgres +
  Qdrant, creates two tenants, asserts isolation, presign roundtrip,
  notification → index, quota enforcement.
- Browser E2E: Playwright spec uploads a 5 MiB file via presign,
  asserts it appears in sidebar and chat retrieval finds it.
- Chaos: kill RustFS mid-upload, confirm `abort` cleans staged parts.

### 4.5 Rollout

1. Land Phase 1 + 2 behind flags off in prod, on in dev/stage.
2. Soak 48h.
3. Flip Phase 2 on in prod for new tenants only.
4. Backfill existing tenants in batches of 50 via a one-shot job
   (provision IAM, no data move — prefix already correct).
5. Land Phase 3, flip after web ships direct-upload UI.
6. Phases 4–8 follow on weekly cadence.

### 4.6 Out of scope

- Lago billing of storage bytes (separate plan).
- Cross-cloud (AWS S3, GCS) abstraction — RustFS only.
- End-to-end client-side encryption (deferred; SSE-KMS covers most needs).

---

## 5. File/crate impact map

| Area | Path | Phase |
|------|------|-------|
| Admin client | `apps/backend/crates/rustfs-admin/**` | 1 |
| Bootstrap | `apps/backend/crates/agent-gateway/src/bootstrap/storage.rs` | 1 |
| TenantContext creds | `apps/backend/crates/agent-core/src/context/tenant.rs` | 2 |
| Content store | `apps/backend/crates/agent-core/src/store/rustfs_content.rs` | 2,4,6 |
| Presign service | `apps/backend/crates/agent-core/src/store/presign.rs` (new) | 3 |
| HTTP routes | `apps/backend/crates/agent-gateway/src/routes/workspaces.rs` + new `uploads.rs` | 3,6 |
| Webhook | `apps/backend/crates/agent-gateway/src/routes/internal.rs` (new) | 5 |
| Indexer | `apps/backend/crates/workspace-indexer/**` | 5 |
| Quota | `apps/backend/crates/agent-core/src/quota.rs` (new) | 6 |
| Metrics | `apps/backend/crates/agent-gateway/src/metrics.rs` | 7 |
| Web upload | `apps/web/src/lib/upload/**` | 3,6 |
| Compose | `docker-compose.yml` (drop `rustfs-init`) | 1 |
| Capability | `apps/backend/capabilities/file-storage/capability.toml` | 3 |
| Docs | `docs/ops/rustfs.md` (new), `docs/arch.md` | all |

---

## 6. Definition of done

- All 8 phases shipped behind flags, defaults as above.
- E2E suite green: tenant isolation, presign roundtrip,
  notification→index, quota, versioning, object-lock (Enterprise).
- Root credential not reachable from any request-handling code path
  (verified by `cargo deny`-style lint + grep).
- DR drill executed once successfully and documented.
- `docs/arch.md` updated to reference this plan; this file marked
  `Status: implemented` with phase completion dates.
