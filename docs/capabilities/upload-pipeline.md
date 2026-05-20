# Upload Pipeline

> How files enter the tenant workspace and become available to capabilities.

---

## Overview

File uploads flow through a two-stage pipeline:

1. **HTTP upload** — bytes are received by the gateway and written to object storage (`TenantStorage`).
2. **Capability post-processing** — the `storage-workspace` capability (or a custom `on_upload` policy) indexes the file, tags it, and emits a realtime event.

```
Client POST /v1/files  (or /ui/upload)
  └─ agent-gateway/ui/handlers/upload.rs
        ├─ parse multipart body
        ├─ TenantStorage::put_object(virtual_path, bytes)
        └─ return { id, download_url, size, content_type }
```

For large files, the gateway exposes a three-step presigned-URL flow:

```
POST /v1/files/uploads/initiate   → upload_id
POST /v1/files/uploads/presign    → presigned part URL (signed by TenantStorage)
POST /v1/files/uploads/complete   → finalize multipart → virtual_path
```

---

## Storage paths

All object paths are constructed by `TenantStorage` and are **never exposed** as raw strings outside that module. The caller works with `VirtualPath` newtypes.

| Layout | Bucket | Key prefix |
|---|---|---|
| `LegacyPrefix` (Phase 1 tenants) | `workspace` (shared) | `tenants/{id}/workspaces/{vp}` |
| `Modern` (Phase 2 tenants) | `ws-{id}` (per-tenant) | `workspaces/{vp}` |

---

## Post-upload indexing

After a file lands in object storage, capabilities can be invoked to process it:

- `sense-mime` — detect MIME type
- `sense-classify-document` — classify as invoice / contract / receipt / etc.
- `storage-tag` — write `.meta.json` sidecar with classification results
- `storage-ensure-date-folder` — ensure a date-partitioned upload directory exists

These are independent `chain` or `native` capabilities and can be orchestrated with `plan_orchestrate` (see [orchestration.md](orchestration.md)).

---

## Adding an `on_upload` policy

To run capabilities automatically after every upload for a tenant:

1. Create a `plan_steps` JSON definition for the policy.
2. Register it via `POST /admin/tenants/{id}/upload_policy` (planned — not yet implemented).
3. The gateway will execute the plan after `TenantStorage::put_object` completes.

This is the mechanism for zero-touch document ingestion pipelines.

---

## Realtime events

After a successful upload, a `file.created` event is broadcast on the tenant's realtime channel (SSE at `GET /v1/realtime`). Clients can subscribe to trigger UI updates without polling.
