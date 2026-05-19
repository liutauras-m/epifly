# MinIO → RustFS Migration Plan

**Status**: Proposed
**Owner**: Platform / Infra
**Created**: 2026-05-19
**Reference**: <https://docs.rustfs.com/installation/docker/>
**Strategy**: Aggressive — full drop-in replacement, no compatibility shims, no rollback path.

---

## 1. Context

Today, the service named `conusai-rustfs` in [docker-compose.yml](docker-compose.yml) actually runs the **MinIO** server image (`quay.io/minio/minio:RELEASE.2025-04-22T22-12-26Z`). "RustFS" is only a naming convention in this repo — there is no real RustFS code in the stack.

The real **[RustFS](https://rustfs.com/)** project is a Rust-native, 100% S3-compatible, Apache-2.0–licensed distributed object store (image `rustfs/rustfs:latest`). It exposes the same S3 protocol on port 9000 with a web console on port 9001, but uses a different env-var prefix (`RUSTFS_*` instead of `MINIO_*`), runs as UID `10001`, and ships no `mc` (MinIO Client) binary inside the image.

Replacing MinIO with the real RustFS aligns the implementation with the name we already use, removes a third-party (and licence-restricted under AGPLv3) dependency, and gives us a Rust-only object-storage runtime that matches the rest of the backend.

---

## 2. Goals & Non-Goals

### Goals
1. Run `rustfs/rustfs:latest` in place of `quay.io/minio/minio` in all environments (dev, CI, prod).
2. Replace every `MINIO_*` env var and `mc`-based bucket bootstrap with the equivalent RustFS-native mechanism.
3. Update every doc, ADR, comment, script, and verification helper so that the word "MinIO" disappears from the repo, and the word "RustFS" refers exclusively to the real product.
4. Keep the existing S3 client code (`object_store::aws::AmazonS3Builder`) unchanged — RustFS is wire-compatible.
5. Keep the on-disk layout `workspace/tenants/{tenant_id}/{file_id}/{filename}` unchanged.
6. Re-run the full Docker verification suite (verify.md Phases 4, 9, 9b, 12) end-to-end and update results.

### Non-Goals
- **No data migration** from existing MinIO volumes — local dev volumes are disposable; production has not yet shipped on MinIO.
- **No TLS / IAM hardening** in this pass — keep the local stack on plain HTTP with a single root credential, matching today's posture.
- **No multi-node / erasure-coded deployment** — we stay on Single-Node Single-Disk (SNSD) for dev; a follow-up ADR will cover production HA.
- **No change** to the application API (`POST /v1/files`, `GET /v1/files/{token}`) or to the `object_store` crate.

---

## 3. Scope — Files & Symbols to Change

The migration touches **five buckets** of code/config. Every item listed here is an explicit edit, not a placeholder.

### 3.1 Docker / Infra
| File | Change |
|------|--------|
| [docker-compose.yml](docker-compose.yml) §`rustfs` service | Swap image to `rustfs/rustfs:latest`; replace `MINIO_ROOT_USER`/`MINIO_ROOT_PASSWORD` with `RUSTFS_ACCESS_KEY`/`RUSTFS_SECRET_KEY`; change command to `--console-enable --address :9000 /data`; replace healthcheck URL `/minio/health/live` with `/` or TCP probe on 9000. |
| [docker-compose.yml](docker-compose.yml) §`rustfs-init` service | **Delete entirely.** RustFS image has no `mc`. Replace bucket-bootstrap with a tiny `amazon/aws-cli`-based init container (or, preferred, fold into the gateway startup — see §3.3). |
| [docker-compose.yml](docker-compose.yml) §`rustfs_perms` (new) | Add an `alpine` init container that `chown -R 10001:10001 /fix_path` against the `rustfs_data` volume mount, with `condition: service_completed_successfully` on the `rustfs` service. |
| [docker-compose.yml](docker-compose.yml) §`agent-gateway` env | Rename defaults: `AWS_ACCESS_KEY_ID=${AWS_ACCESS_KEY_ID:-rustfsadmin}` and `AWS_SECRET_ACCESS_KEY=${AWS_SECRET_ACCESS_KEY:-rustfsadmin}`. |
| [apps/backend/start.sh](apps/backend/start.sh) line 81 | Replace `curl …/minio/health/live` with `curl -sf http://localhost:9000/` (or TCP wait on 9000). |
| [apps/backend/Dockerfile](apps/backend/Dockerfile) | No change (we keep `object_store` 0.11 with `aws` feature). |

### 3.2 Bucket Bootstrap
RustFS's official image does **not** ship `mc`. Options considered:

| Option | Verdict |
|--------|---------|
| Run `rustfs/rustfs` as init with a CLI subcommand | ❌ No documented bucket-create subcommand in the SNSD container today |
| Use `amazon/aws-cli` against the new `rustfs:9000` endpoint | ✅ **Chosen.** Same image already used in [verify.md Phase 9](docs/verify/verify.md), AGPL-free, ~80 MB |
| Move bucket bootstrap inside the gateway startup (`init_file_store`) | ⚠️ Acceptable fallback. Would call `PutBucket` via `object_store` extension API if the bucket is missing — but `object_store` 0.11 does not expose `CreateBucket`. Defer to a separate ticket. |

**Decision**: replace `rustfs-init` with an `aws-cli` container that calls `s3 mb s3://workspace`, using `--endpoint-url http://conusai-rustfs:9000`.

### 3.3 Rust Source
| File | Change |
|------|--------|
| [Cargo.toml](Cargo.toml) line 79 comment | `# Object / file storage (MinIO / S3)` → `# Object / file storage (S3 — RustFS in dev, AWS S3 in prod)`. |
| `apps/backend/crates/agent-gateway/src/state.rs` `init_file_store` | Change `unwrap_or_else` defaults: `S3_ENDPOINT="http://rustfs:9000"` (unchanged hostname — Docker DNS resolves to new container), `AWS_ACCESS_KEY_ID="rustfsadmin"`, `AWS_SECRET_ACCESS_KEY="rustfsadmin"`. Drop any inline references to "minio" in log messages. |
| `apps/backend/crates/agent-gateway/src/bin/memory_migrate.rs` doc comment | Update example env vars to RustFS naming. |
| Tests under `apps/backend/crates/*/tests/` referencing `minioadmin` (none verified — sweep with `rg minioadmin`) | Replace literal with `rustfsadmin`. |

### 3.4 Documentation
| File | Change |
|------|--------|
| [docs/arch.md](docs/arch.md) line 96 | `RustFS / S3 / MinIO content store` → `RustFS / AWS S3 content store`. |
| [docs/arch.md](docs/arch.md) line 275 | `# object_store (S3/MinIO)` → `# object_store (S3 / RustFS)`. |
| [docs/arch.md](docs/arch.md) line 475 | `RustFS / MinIO / S3 (object_store 0.11)` → `RustFS / AWS S3 (object_store 0.11)`. |
| [docs/arch.md](docs/arch.md) — new §10 footnote | Add note: "Object storage is the real [RustFS](https://rustfs.com) (`rustfs/rustfs:latest`) running in SNSD mode; AGPLv3-licensed MinIO has been removed from the stack." |
| [docs/project-instructions.md](docs/project-instructions.md) line 77 | `object_store 0.11 (aws/S3/MinIO)` → `object_store 0.11 (aws/S3/RustFS)`. |
| [docs/project-instructions.md](docs/project-instructions.md) line 108 | `Multipart file upload (MinIO-backed)` → `Multipart file upload (RustFS-backed)`. |
| [docs/capability-gaps-pan.md](docs/capability-gaps-pan.md) line 264 | `MINIO_*` reference → `RUSTFS_*` (or drop section if obsolete). |
| [docs/verify/verify.md](docs/verify/verify.md) Phase 9, 9b, 12.3 | Replace `minioadmin/minioadmin` credentials with `rustfsadmin/rustfsadmin`; update Phase 9b "MinIO console" → "RustFS console"; replace `quay.io/minio/minio` example with `rustfs/rustfs:latest`. Update Coverage Assessment row to say **"RustFS object storage — console verification"**. |
| [docs/adr/0009-redb-qdrant-rustfs.md](docs/adr/0009-redb-qdrant-rustfs.md) | Add an **amendment** section dated 2026-05-19: "Object storage backend swapped from MinIO container (placeholder) to the real RustFS server image (`rustfs/rustfs:latest`). Wire protocol (S3) and on-disk layout are unchanged." |

### 3.5 Environment & Helper Scripts
| File | Change |
|------|--------|
| `.env.example` (create if missing) | Document `AWS_ACCESS_KEY_ID=rustfsadmin`, `AWS_SECRET_ACCESS_KEY=rustfsadmin`, `S3_ENDPOINT=http://rustfs:9000`, `S3_BUCKET=workspace`. |
| Any `start.sh` / `stop.sh` referencing `minio` | Sweep and rename. |
| Existing `.env.local` (developer machines) | Document in PR notes that devs must `docker volume rm conusai-platform_rustfs_data` once before bringing the new stack up (MinIO and RustFS use incompatible on-disk metadata). |

---

## 4. Target docker-compose Snippet

```yaml
  # ── Object storage permissions fix (RustFS runs as UID 10001) ────────────
  rustfs-perms:
    image: alpine:3
    container_name: conusai-rustfs-perms
    user: root
    volumes:
      - rustfs_data:/fix_path
    command: chown -R 10001:10001 /fix_path
    restart: "no"

  # ── Object Storage — RustFS (S3-compatible, Apache 2.0) ───────────────────
  rustfs:
    image: rustfs/rustfs:latest
    container_name: conusai-rustfs
    depends_on:
      rustfs-perms:
        condition: service_completed_successfully
    environment:
      RUSTFS_ACCESS_KEY: "${AWS_ACCESS_KEY_ID:-rustfsadmin}"
      RUSTFS_SECRET_KEY: "${AWS_SECRET_ACCESS_KEY:-rustfsadmin}"
      RUSTFS_ADDRESS: ":9000"
      RUSTFS_CONSOLE_ENABLE: "true"
    command: ["--console-enable", "/data"]
    ports:
      - "9000:9000"   # S3 API
      - "9001:9001"   # Web console
    volumes:
      - rustfs_data:/data
    healthcheck:
      test: ["CMD", "bash", "-lc", "echo > /dev/tcp/127.0.0.1/9000"]
      interval: 10s
      timeout: 5s
      retries: 10

  # ── Bucket bootstrap (RustFS image has no `mc`) ───────────────────────────
  rustfs-init:
    image: amazon/aws-cli:latest
    container_name: conusai-rustfs-init
    depends_on:
      rustfs:
        condition: service_healthy
    environment:
      AWS_ACCESS_KEY_ID: "${AWS_ACCESS_KEY_ID:-rustfsadmin}"
      AWS_SECRET_ACCESS_KEY: "${AWS_SECRET_ACCESS_KEY:-rustfsadmin}"
      AWS_DEFAULT_REGION: us-east-1
    entrypoint:
      - /bin/sh
      - -c
      - |
        aws --endpoint-url http://conusai-rustfs:9000 s3 mb s3://workspace 2>/dev/null || true
        aws --endpoint-url http://conusai-rustfs:9000 s3 ls
    restart: on-failure
```

---

## 5. Execution Plan (Phases)

> All phases are **aggressive** — no feature flags, no parallel "old + new" running, no opt-in env var.

### Phase 0 — Pre-flight (5 min)
- [ ] Snapshot current `docker-compose ps` and `docker volume ls` for the record.
- [ ] Confirm no production traffic is hitting `conusai-rustfs` (this stack is local/dev-only today).
- [ ] Pull `rustfs/rustfs:latest` and confirm it starts standalone: `docker run --rm -p 9000:9000 -p 9001:9001 rustfs/rustfs:latest /data` → expect log "listening on :9000".

### Phase 1 — Code & Config Changes (single commit)
- [ ] Apply every edit in §3.1, §3.3, §3.5 above.
- [ ] Run `cargo check --workspace` — expect zero changes (S3 wire protocol unchanged).
- [ ] Run `cargo test --workspace` — expect green (no test references MinIO directly).

### Phase 2 — Local Bring-up (10 min)
- [ ] `docker compose down -v` (destroys existing MinIO volume — **destructive but acceptable**, dev data only).
- [ ] `docker compose up -d --build`.
- [ ] Verify `docker compose ps` shows `conusai-rustfs` healthy and `conusai-rustfs-init` exited 0.
- [ ] Browse <http://localhost:9001> → log in with `rustfsadmin`/`rustfsadmin` → confirm `workspace` bucket exists.

### Phase 3 — Re-run verify.md Phase 9 + 9b
- [ ] Re-execute the Python upload/download script from the last verification run (terminal scrollback).
- [ ] Expected: `POST /v1/files` returns a UUID token; `GET /v1/files/{token}` returns the PNG; the file is visible in the RustFS console at `workspace/tenants/acme/{uuid}/invoice.png`.
- [ ] Re-run `aws --endpoint-url http://conusai-rustfs:9000 s3 ls s3://workspace/ --recursive` → expect identical hierarchy.

### Phase 4 — Documentation Sweep
- [ ] Apply every edit in §3.4 above (arch.md, project-instructions.md, verify.md, capability-gaps-pan.md, ADR 0009 amendment).
- [ ] `rg -i 'minio'` across the repo → expect **zero** matches except inside `docs/rustfs-plan.md` itself (this file) and historic ADR amendment context.
- [ ] `rg -i 'minioadmin'` → expect **zero** matches.

### Phase 5 — CI / Branch Validation
- [ ] Push branch, let CI rebuild Docker images and run the full verify suite.
- [ ] Block merge until: (a) `docker compose up` cleanly bootstraps, (b) verify Phase 9 passes, (c) `rg -i minio` returns clean.

### Phase 6 — Merge & Announce
- [ ] Merge.
- [ ] Post in `#eng-platform`: "MinIO removed. Run `docker compose down -v && docker compose up -d` once to refresh your local stack. Console creds are now `rustfsadmin`/`rustfsadmin`."

---

## 6. Risks & Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| RustFS image incompatible with `object_store` 0.11 SigV4 quirks | Low | High (uploads fail) | Tested locally before merge; both products claim 100% S3 compatibility for SNSD mode |
| `rustfs/rustfs:latest` lacks healthcheck endpoint | Medium | Medium (compose marks unhealthy) | Use TCP probe on port 9000 instead of HTTP path |
| UID 10001 permission errors on bind mounts | High on first run | Low (init container fixes it) | `rustfs-perms` init container handles it; documented in Phase 6 announcement |
| Dev volumes corrupted (MinIO data dir not portable) | Certain | Low | Aggressive plan accepts `docker compose down -v` — local dev data is disposable |
| `mc` muscle memory in onboarding docs | Medium | Low | Replace every `mc` example with `aws --endpoint-url` |
| RustFS console UI behaves differently than MinIO console | Certain | Low | Update verify.md Phase 9b screenshots/steps in Phase 4 |
| Bucket auto-creation path differs (no `mc mb`) | Certain | Low | Use `aws s3 mb` from `amazon/aws-cli` init container |

---

## 7. Acceptance Criteria

A passing migration meets **all** of the following:

1. ✅ `docker compose up -d` brings up `conusai-rustfs` (image `rustfs/rustfs:latest`) healthy in ≤ 30 s.
2. ✅ `workspace` bucket is auto-created on first run.
3. ✅ `curl -sf -X POST http://localhost:8080/v1/files -H "Authorization: Bearer $TOKEN" -F file=@invoice.png` returns a JSON body with a UUID `id`.
4. ✅ `curl -sf http://localhost:8080/v1/files/{id} -o /tmp/x.png` followed by `file /tmp/x.png` reports `PNG image data`.
5. ✅ The RustFS console at <http://localhost:9001> shows the uploaded file under `workspace/tenants/acme/{uuid}/invoice.png`.
6. ✅ `cargo test --workspace` is green.
7. ✅ `rg -i '(minio|minioadmin)'` returns zero matches outside [docs/rustfs-plan.md](docs/rustfs-plan.md) and the ADR amendment.
8. ✅ [docs/arch.md](docs/arch.md), [docs/project-instructions.md](docs/project-instructions.md), [docs/verify/verify.md](docs/verify/verify.md), and [docs/adr/0009-redb-qdrant-rustfs.md](docs/adr/0009-redb-qdrant-rustfs.md) all reflect the new reality.

---

## 8. Post-Migration arch.md Updates (preview)

After the cutover, the following sections in [docs/arch.md](docs/arch.md) must read:

- **§ Dependency table**: `object_store` 0.11 (`aws`) — *S3-compatible object store (RustFS in dev, AWS S3 in prod)*.
- **§ Storage tree**: `rustfs_content.rs   # object_store (S3 / RustFS)`.
- **§ Storage layer matrix**: Object content — *RustFS (rustfs/rustfs) / AWS S3 (object_store 0.11)* — `store/rustfs_content.rs`.
- **§10 (new bullet)**: *"Local object storage runs the real [RustFS](https://rustfs.com) server (Apache-2.0, `rustfs/rustfs:latest`). MinIO is no longer part of the stack."*
- **§ ADR cross-ref**: link to ADR 0009 amendment 2026-05-19.

---

## 9. Out of Scope (Follow-up Tickets)

- Multi-node / erasure-coded RustFS for production (new ADR).
- TLS termination on RustFS S3 endpoint.
- RustFS IAM tokens per tenant (replace shared `rustfsadmin` root key).
- Moving bucket bootstrap from a sidecar container into the gateway's `init_file_store` once `object_store` exposes `CreateBucket` (or via a thin `aws-sdk-s3` dependency).
- Versioning / lifecycle policy on the `workspace` bucket.
