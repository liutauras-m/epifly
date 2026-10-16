# RustFS Operations Runbook

## Environment variables

The gateway reads these through its environment contract
(`apps/backend/crates/agent-gateway/src/config.rs`); a missing required variable stops it
at start with exit code 2. On ConusAI Cloud the linked S3 service injects `S3_BUCKET` and
the `AWS_*` variables; locally `docker-compose.local.yml` runs RustFS with the same names.
Every project variable is listed in [deploy-conusai-cloud.md](../deploy-conusai-cloud.md).

| Var | Required | Purpose |
|-----|----------|---------|
| `AWS_ENDPOINT_URL` | yes | S3 endpoint (locally `http://rustfs:9000`) |
| `AWS_ACCESS_KEY_ID` | yes | Admin credential: bootstrap and per-tenant IAM provisioning |
| `AWS_SECRET_ACCESS_KEY` | yes | Admin credential |
| `S3_BUCKET` | yes | Shared bucket |
| `RUSTFS_IAM_ENC_KEY` | yes | Base64 of exactly 32 bytes: AES-256-GCM key for the stored per-tenant credentials. Never regenerate on a running instance |
| `RUSTFS_NOTIFICATION_WEBHOOK_URL` | no | URL of `/internal/rustfs/events` reachable from the S3 server; set together with `RUSTFS_WEBHOOK_SECRET` or not at all |
| `RUSTFS_WEBHOOK_SECRET` | no | HMAC secret for the bucket notification webhook |
| `RUSTFS_BOOTSTRAP` | no | `on` (default) / `off`: declarative bucket bootstrap at start |
| `RUSTFS_SSE` | no | `on` (default): default SSE-S3 encryption on the bucket |
| `RUSTFS_VERSIONING` | no | `on` (default): bucket versioning |
| `RUSTFS_QUOTAS` | no | `on` (default): per-tenant storage quotas |
| `RUSTFS_PRESIGN_TTL_SECS` | no | `900` (default, capped at 3600) |
| `RUSTFS_PUBLIC_BASE_URL` | no | Public base URL used in presigned URLs handed to browsers |
| `RUSTFS_KEY_ROTATION_DAYS` | no | `90` (default): age at which the rotation job replaces a tenant's keys |

## Generating `RUSTFS_IAM_ENC_KEY`

```bash
openssl rand -base64 32
```

Store in your secrets manager. Rotation requires re-encrypting all per-tenant credentials (see below).

## Bootstrap

On every gateway start the `bootstrap_storage` function:
1. Creates the `S3_BUCKET` bucket if it doesn't exist (`PUT /{bucket}`)
2. Enables versioning (`PUT /{bucket}?versioning`)
3. Sets lifecycle rules (tmp uploads 24h, exports 7d, old versions 90d)
4. Sets CORS for web origins
5. Configures the bucket notification webhook (only when `RUSTFS_NOTIFICATION_WEBHOOK_URL` is set)

Each step logs a warning and continues when the server refuses it. To disable: `RUSTFS_BOOTSTRAP=off`

## Per-tenant IAM

Each tenant gets a RustFS service account (access key + secret key) via the RustFS admin API. These are:
- Created by `rustfs_admin::provision_tenant()` on first use
- Stored encrypted (AES-256-GCM) in redb at `iam/tenant/{tenant_id}`
- Cached in memory for 5 minutes (max 1024 entries)
- Scoped to `tenants/{tenant_id}/*` via inline IAM policy

### Key rotation

The scheduled job `rustfs-key-rotation` (`apps/backend/crates/jobs/src/jobs/rustfs_key_rotation.rs`)
runs daily at 03:00 and replaces the keys of every tenant whose credentials are older than
`RUSTFS_KEY_ROTATION_DAYS`, keeping the previous access key briefly for in-flight requests.
Run it on demand as a super-admin: `POST /admin/jobs/rustfs-key-rotation/run`.

## Presigned URLs

All uploads/downloads use real S3 SigV4 presigned URLs:
- `POST /v1/workspaces/{id}/presign-upload` → returns presigned PUT URL (15 min default)
- `GET /v1/workspaces/{id}/presign-download?virtual_path=` → returns presigned GET URL
- `POST /v1/files/upload-url` → general presigned upload
- `GET /v1/files/download-url?virtual_path=` → general presigned download

Browsers PUT directly to RustFS — no gateway proxy.

## Storage quotas

| Plan | Limit |
|------|-------|
| Free | 1 GiB |
| Pro | 100 GiB |
| Enterprise | Unlimited |

Quotas are checked at presign time. Usage is aggregated via `ListObjectsV2` and cached for 60s.

To disable: `RUSTFS_QUOTAS=off`

## Versioning

Bucket versioning is enabled by default. API:
- `GET /v1/workspaces/nodes/{id}/versions` — list versions
- `POST /v1/workspaces/nodes/{id}/restore` — restore a version

Old non-current versions expire after 90 days (lifecycle rule).

## Event-driven indexing

RustFS sends S3 event notifications to `POST /internal/rustfs/events`. The gateway:
1. Verifies HMAC (`X-RustFS-Signature: sha256=...` header, `RUSTFS_WEBHOOK_SECRET`)
2. Parses S3 event records
3. For `ObjectCreated`: reads the object, chunks/embeds, upserts to Qdrant
4. For `ObjectRemoved`: deletes vectors from Qdrant by `node_id`

Event-driven indexing is on only when both `RUSTFS_NOTIFICATION_WEBHOOK_URL` and
`RUSTFS_WEBHOOK_SECRET` are set; without them the `/internal/rustfs/events` route is not
mounted.

## Metrics

`GET /metrics` (Prometheus text, requires `Authorization: Bearer <PLATFORM_ADMIN_TOKEN>`)
and the OTLP export carry the storage-related series registered in
`apps/backend/crates/agent-gateway/src/metrics.rs`: `tenant_onboarding_total{kind}`,
`tenant_onboarding_marker_failed_total` and `embedding_dims{model}`. Per-operation S3
latency, error and byte counters are not instrumented.

## Backup and DR

### Nightly backup

```bash
# Run nightly (cron on a separate host with access to both RustFS clusters)
aws --endpoint-url $PRIMARY_S3 s3 sync s3://$S3_BUCKET s3://$S3_BUCKET-backup \
    --endpoint-url-dest $BACKUP_S3 \
    --delete \
    --sse aws:kms
```

### Point-in-time recovery

Versioning + lifecycle provide PITR within the 90-day retention window. Tenants on the
shared-bucket layout live under `tenants/{tenant_id}/workspaces/` in `S3_BUCKET`; tenants
moved by the `tenant-bucket-migration` job live under `workspaces/` in their own bucket
`ws-{tenant_id}`.
```bash
# List versions of a specific object
aws --endpoint-url $S3 s3api list-object-versions \
    --bucket $S3_BUCKET \
    --prefix "tenants/{tenant_id}/workspaces/{virtual_path}"

# Restore by copying a version
aws --endpoint-url $S3 s3 cp \
    "s3://$S3_BUCKET/{key}?versionId={version_id}" \
    "s3://$S3_BUCKET/{key}"
```

### DR drill

```bash
# 1. Start an empty S3 server (e.g. the rustfs service of docker-compose.local.yml)

# 2. Restore from backup
aws --endpoint-url $BACKUP_S3 s3 sync s3://$S3_BUCKET-backup s3://$S3_BUCKET \
    --endpoint-url-dest $DR_S3

# 3. Boot gateway against DR cluster
#    (in .env: AWS_ENDPOINT_URL=$DR_S3, then `just up`)

# 4. Run the tenant isolation tests against it (ignored unless RUSTFS_TEST_ENDPOINT is set)
RUSTFS_TEST_ENDPOINT=$DR_S3 cargo test -p agent-gateway --test tenant_isolation -- --include-ignored

# 5. Verify RPO < 24h: check most recent object timestamp
aws --endpoint-url $DR_S3 s3 ls s3://$S3_BUCKET --recursive \
    | sort | tail -5

# 6. Document RTO (time from step 1 to smoke tests passing)
```

RTO target: < 1h. RPO target: < 24h (nightly backup cadence).

## Security pen-test checklist

- [ ] Cross-tenant prefix: `GET tenants/other-tenant/workspaces/doc` returns 403
- [ ] Path traversal: `virtual_path=../../etc/passwd` returns 400
- [ ] Oversized presign TTL: `RUSTFS_PRESIGN_TTL_SECS=99999` caps at 3600
- [ ] Replay presigned URL after key rotation: returns SignatureDoesNotMatch
- [ ] Root credential not accessible from request-handling code
- [ ] Expired presigned URL: returns 403 from RustFS
- [ ] Quota enforcement: uploading beyond plan limit returns HTTP 413
- [ ] HMAC webhook replay: old event with valid HMAC replayed → idempotent, no error
- [ ] Webhook with wrong HMAC → 401
