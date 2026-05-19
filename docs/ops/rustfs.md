# RustFS Operations Runbook

## Environment variables

| Var | Required in prod | Purpose |
|-----|-----------------|---------|
| `S3_ENDPOINT` | yes | RustFS S3 endpoint (`http://rustfs:9000`) |
| `S3_BUCKET` | yes | Primary bucket (`workspace`) |
| `RUSTFS_ROOT_ACCESS_KEY` | yes | Root admin credential |
| `RUSTFS_ROOT_SECRET_KEY` | yes | Root admin credential |
| `RUSTFS_IAM_ENC_KEY` | yes | 32-byte AES-256-GCM key (base64) for per-tenant credential encryption |
| `RUSTFS_WEBHOOK_SECRET` | yes | HMAC secret for bucket notification webhook |
| `RUSTFS_NOTIFICATION_WEBHOOK_URL` | yes | URL of `/internal/rustfs/events` reachable from RustFS |
| `RUSTFS_BOOTSTRAP` | no | `on` (default) — run declarative bootstrap on startup |
| `RUSTFS_PER_TENANT_IAM` | no | `on` (default) — use per-tenant S3 credentials |
| `RUSTFS_REAL_PRESIGN` | no | Always `on` — presigned URLs are always real SigV4 |
| `RUSTFS_SSE` | no | `on` (default) — SSE-S3 on all writes |
| `RUSTFS_VERSIONING` | no | `on` (default) — bucket versioning |
| `RUSTFS_NOTIFICATIONS` | no | `on` (default) — event-driven indexing |
| `RUSTFS_QUOTAS` | no | `on` (default) — per-tenant storage quotas |
| `RUSTFS_PRESIGN_TTL_SECS` | no | `900` (default, max 3600) |

## Generating `RUSTFS_IAM_ENC_KEY`

```bash
openssl rand -base64 32
```

Store in your secrets manager. Rotation requires re-encrypting all per-tenant credentials (see below).

## Bootstrap

On every gateway start the `bootstrap_storage` function:
1. Creates the `workspace` bucket if it doesn't exist (`PUT /{bucket}`)
2. Enables versioning (`PUT /{bucket}?versioning`)
3. Sets lifecycle rules (tmp uploads 24h, exports 7d, old versions 90d)
4. Sets CORS for web origins
5. Configures bucket notification webhook

To disable: `RUSTFS_BOOTSTRAP=off`

## Per-tenant IAM

Each tenant gets a RustFS service account (access key + secret key) via the MinIO admin API. These are:
- Created by `rustfs_admin::provision_tenant()` on first use
- Stored encrypted (AES-256-GCM) in redb at `iam/tenant/{tenant_id}`
- Cached in memory for 5 minutes (max 1024 entries)
- Scoped to `tenants/{tenant_id}/*` via inline IAM policy

### Key rotation

```bash
# 1. Generate new credentials via admin API
# 2. Store new creds in redb (gateway will pick them up)
# 3. After grace period, delete old access key:
curl -X DELETE http://gateway:8080/admin/rustfs/rotate/{tenant_id}
```

To disable per-tenant IAM (dev only): `RUSTFS_PER_TENANT_IAM=off`

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

To disable: `RUSTFS_NOTIFICATIONS=off`

## Metrics

All RustFS metrics are exposed at `/metrics`:

```
rustfs_op_latency_seconds{op, tenant}    — histogram
rustfs_op_errors_total{op, code, tenant} — counter
rustfs_bytes_in_total{tenant}            — counter
rustfs_bytes_out_total{tenant}           — counter
rustfs_storage_used_bytes{tenant}        — gauge
```

## Backup and DR

### Nightly backup

```bash
# Run nightly (cron on a separate host with access to both RustFS clusters)
aws --endpoint-url $PRIMARY_S3 s3 sync s3://workspace s3://workspace-backup \
    --endpoint-url-dest $BACKUP_S3 \
    --delete \
    --sse aws:kms
```

### Point-in-time recovery

Versioning + lifecycle provide PITR within the 90-day retention window:
```bash
# List versions of a specific object
aws --endpoint-url $S3 s3api list-object-versions \
    --bucket workspace \
    --prefix "tenants/{tenant_id}/workspaces/{virtual_path}"

# Restore by copying a version
aws --endpoint-url $S3 s3 cp \
    "s3://workspace/{key}?versionId={version_id}" \
    "s3://workspace/{key}"
```

### DR drill (`make rustfs-dr-drill`)

```bash
# 1. Spin up fresh RustFS cluster
docker compose -f docker-compose.dr.yml up rustfs

# 2. Restore from backup
aws --endpoint-url $BACKUP_S3 s3 sync s3://workspace-backup s3://workspace \
    --endpoint-url-dest $DR_S3

# 3. Boot gateway against DR cluster
S3_ENDPOINT=$DR_S3 cargo run -p agent-gateway

# 4. Run smoke tests
cargo test -p agent-gateway --test tenant_isolation

# 5. Verify RPO < 24h: check most recent object timestamp
aws --endpoint-url $DR_S3 s3 ls s3://workspace --recursive \
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
