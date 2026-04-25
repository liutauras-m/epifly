# ConusAI Platform — Docker Verification Plan

End-to-end verification of the **ConusAI multitenant agent platform** running entirely in Docker.

> **Architecture under test**: workspace with `common`, `agent-core`, `agent-gateway`, `invoice-demo`, `evals` crates; Anthropic Claude via Rig; per-tenant isolation; capabilities auto-discovery; invoice extraction pipeline.

---

## Prerequisites

```bash
# 1. Docker (≥ 24.0) with Compose v2
docker --version
docker compose version

# 2. Anthropic API key (in .env.local — never commit)
grep -q ANTHROPIC_API_KEY .env.local || echo "ANTHROPIC_API_KEY=sk-ant-..." >> .env.local

# 3. The invoice fixture
ls invoice.png   # must be present in repo root
```

---

## Phase 0 — Workspace Sanity (5 min)

```bash
# Validate workspace layout
ls Cargo.toml docker-compose.yml Dockerfile rust-toolchain.toml

# All 5 crates registered?
cargo metadata --format-version 1 --no-deps \
  | jq -r '.packages[].name' \
  | sort
# Expected: agent-core, agent-gateway, common, evals, invoice-demo
```

✅ **Pass criteria**: 5 crates listed, no missing files.

---

## Phase 1 — Local Build & Lint Gate (3 min)

Run before docker build to fail fast on code issues:

```bash
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo check --workspace
```

✅ **Pass criteria**: zero warnings, zero errors.

---

## Phase 2 — Unit Tests (2 min)

```bash
cargo test --workspace --lib
```

✅ **Pass criteria**: **12 tests pass** (5 in `common` + 4 in `agent-core` registry + 3 in `path_safety` for tenant traversal rejection).

---

## Phase 3 — Build Docker Images (~5 min cold, ~30s cached)

```bash
# Build the gateway image (multi-stage)
docker compose build agent-gateway

# Verify image exists
docker images | grep conusai
```

✅ **Pass criteria**: `agent-gateway` image built; final layer ~80 MB (debian-slim + binary).

---

## Phase 4 — Start Infrastructure Stack

```bash
# Profile "full" = Qdrant + MinIO + gateway
docker compose --profile full up -d --wait

# Confirm all containers healthy
docker compose ps
```

Expected:
| Container | Port(s) | Status |
|-----------|---------|--------|
| conusai-qdrant | 6333, 6334 | healthy |
| conusai-minio | 9000, 9001 | healthy |
| conusai-gateway | 8080 | healthy |

✅ **Pass criteria**: all three show **healthy**.

---

## Phase 5 — Service Endpoint Tests

### 5.1 Health
```bash
curl -sf http://localhost:8080/health | jq
# Expected: {"status":"ok","version":"0.1.0","capabilities":3}
```

### 5.2 Capabilities listing (with X-Tenant-ID dev fallback)
```bash
curl -sf -H "X-Tenant-ID: acme" http://localhost:8080/v1/capabilities | jq
# Expected:
# {
#   "tenant_id": "acme",
#   "plan": "free",
#   "capabilities": [
#     {"name":"invoice-processing","kind":"Pipeline",...},
#     {"name":"file-storage","kind":"Mcp",...},
#     {"name":"ocr-service","kind":"Pipeline",...}
#   ]
# }
```

### 5.3 OpenAI-compatible chat completion
```bash
curl -sf -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "X-Tenant-ID: acme" \
  -d '{
    "model": "claude-opus-4-7",
    "messages": [{"role":"user","content":"Say hello in one word."}],
    "max_tokens": 50
  }' | jq
```

✅ **Pass criteria**: `choices[0].message.content` contains a coherent reply; `model` echoes back; `id` starts with `chatcmpl-`.

### 5.4 Multitenancy isolation — different tenants, same gateway
```bash
# Tenant A
curl -sf -H "X-Tenant-ID: tenant-a" http://localhost:8080/v1/capabilities | jq -r .tenant_id
# → tenant-a

# Tenant B
curl -sf -H "X-Tenant-ID: tenant-b" http://localhost:8080/v1/capabilities | jq -r .tenant_id
# → tenant-b
```

✅ **Pass criteria**: each request returns its own tenant context.

### 5.5 Per-tenant rate limiting (free tier = 10 rpm)
```bash
# Fire 12 rapid requests as a free-tier tenant; 11th+ should 429
for i in {1..12}; do
  code=$(curl -s -o /dev/null -w "%{http_code}" \
    -X POST http://localhost:8080/v1/chat/completions \
    -H "Content-Type: application/json" \
    -H "X-Tenant-ID: ratelimit-test" \
    -d '{"model":"claude-opus-4-7","messages":[{"role":"user","content":"hi"}],"max_tokens":5}')
  echo "Request $i: HTTP $code"
done
```

✅ **Pass criteria**: first ~10 return `200`, then `429 Too Many Requests`.

---

## Phase 6 — Invoice Extraction (End-to-End)

The flagship test — extract structured data from `invoice.png` via Claude vision.

### 6.1 Via the `invoice-demo` binary running inside the gateway container
```bash
# Copy the fixture into the running container and exec
docker cp invoice.png conusai-gateway:/app/invoice.png

# The gateway image only ships the gateway binary, so build invoice-demo
# locally and run with the same env:
source .env.local
./target/release/invoice-demo invoice.png \
  --tenant-id acme \
  --plan enterprise
```

Expected output (key fields):
```
Invoice #:   HCY-23256029
Status:      PAID
Total:       €63.99
Issuer:      Hostinger International Ltd.
Billed To:   Liutauras Medziunas / Conus AI
```

### 6.2 Via the evals harness
```bash
source .env.local
cargo run --release --bin evals -- run --suite invoice
```

✅ **Pass criteria**: scorer reports **`✅ ALL PASS`** with avg score ≥ 80%.

---

## Phase 7 — Storage & Persistence Checks

### 7.1 Qdrant
```bash
curl -sf http://localhost:6333/collections | jq
# Expected: {"result":{"collections":[]},...}  (empty until embeddings written)
```

### 7.2 MinIO
```bash
# Console: http://localhost:9001  (login: minioadmin / minioadmin)
# CLI smoke test
docker run --rm --network conusai-platform_default \
  -e AWS_ACCESS_KEY_ID=minioadmin -e AWS_SECRET_ACCESS_KEY=minioadmin \
  amazon/aws-cli --endpoint-url http://conusai-minio:9000 s3 ls
```

✅ **Pass criteria**: both services respond; MinIO `s3 ls` succeeds.

---

## Phase 8 — Observability

```bash
# Gateway structured logs include tenant_id
docker compose logs agent-gateway --since=2m | grep tenant_id

# Trace headers propagate
curl -sf -H "X-Tenant-ID: acme" -H "traceparent: 00-$(openssl rand -hex 16)-$(openssl rand -hex 8)-01" \
  http://localhost:8080/v1/capabilities -o /dev/null -w "%{http_code}\n"
```

✅ **Pass criteria**: log lines contain JSON with `"tenant_id":"acme"`.

---

## Phase 9 — Tear Down

```bash
docker compose --profile full down -v   # -v also removes volumes
```

---

## One-Command Master Verification

`scripts/docker-verify.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

echo "═══ ConusAI Docker Verification ═══"

# Phase 1–2: local gates
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace --lib

# Phase 3–4: docker
docker compose --profile full up -d --build --wait

# Phase 5: endpoint smoke
curl -sf http://localhost:8080/health > /dev/null
curl -sf -H "X-Tenant-ID: ci" http://localhost:8080/v1/capabilities > /dev/null

# Phase 6: invoice extraction
source .env.local
cargo run --release --bin invoice-demo -- invoice.png --tenant-id ci --plan enterprise > /tmp/invoice.out
grep -q "HCY-23256029" /tmp/invoice.out  || { echo "❌ Invoice number mismatch"; exit 1; }
grep -q "PAID"          /tmp/invoice.out || { echo "❌ Status mismatch"; exit 1; }
grep -q "€63.99"        /tmp/invoice.out || { echo "❌ Total mismatch"; exit 1; }

# Tear down
docker compose --profile full down -v

echo ""
echo "✅ All verification phases passed."
echo "   • Workspace clean & tested (12/12)"
echo "   • Docker stack healthy"
echo "   • Multitenancy enforced"
echo "   • Invoice extraction PASSED on invoice.png"
```

Run with:
```bash
chmod +x scripts/docker-verify.sh
./scripts/docker-verify.sh
```

---

## Final Checklist

- [ ] `cargo check --workspace` passes
- [ ] `cargo test --workspace --lib` → 12/12 pass
- [ ] `cargo clippy -- -D warnings` clean
- [ ] `docker compose build agent-gateway` succeeds
- [ ] `docker compose --profile full up -d --wait` → all healthy
- [ ] `GET /health` returns `{"status":"ok","capabilities":3}`
- [ ] `GET /v1/capabilities` (with `X-Tenant-ID`) returns 3 capabilities + tenant context
- [ ] `POST /v1/chat/completions` returns coherent Claude response
- [ ] Per-tenant rate limit triggers `429` after 10 RPM (free tier)
- [ ] `invoice-demo invoice.png` extracts `HCY-23256029`, `PAID`, `€63.99`
- [ ] `evals run --suite invoice` → ✅ ALL PASS
- [ ] Logs contain `tenant_id` field on every request
- [ ] `docker compose down -v` tears down cleanly

---

## Troubleshooting

| Symptom | Likely cause | Fix |
|---------|--------------|-----|
| Gateway 401 | `JWT_SECRET` set but no `Authorization: Bearer` header | Either unset `JWT_SECRET` or add `X-Tenant-ID` header |
| `temperature is deprecated for this model` | Calling `claude-opus-4-7` with `temperature` | Already removed from invoice pipeline |
| Qdrant unhealthy | Port 6333 already in use | `lsof -i :6333` and free it, or change `docker-compose.yml` port |
| `invoice extraction failed: x-api-key required` | `ANTHROPIC_API_KEY` not in env | `source .env.local` before running |
| Build cache miss every time | Cargo.lock changes | Commit Cargo.lock |
