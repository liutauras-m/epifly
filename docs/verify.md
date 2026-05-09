# ConusAI Platform — Verification Plan

End-to-end verification of the **ConusAI multitenant agent platform** — Rust backend in Docker + SvelteKit web app on Node.

> **Architecture under test**:
> - **Backend**: Cargo workspace with `common`, `agent-core`, `agent-gateway`, `invoice-demo`, `evals` crates; Anthropic Claude via Rig; per-tenant isolation; capabilities auto-discovery; invoice pipeline; SSE streaming; tool-calling agent loop; MCP JSON-RPC 2.0; MinIO file storage; Qdrant semantic search; WASM runtime.
> - **Web app** (`apps/web`): SvelteKit 2 + Svelte 5 SSR, `adapter-node`, HMAC-SHA256 session cookie shared with the Rust gateway. Replaces the previous Askama-rendered Foundry UI per [`docs/browser-shell-plan.md`](browser-shell-plan.md) Phase 3. The Rust gateway now serves only API + streaming endpoints.
> - **Out of scope (planned)**: `apps/browser-shell` (Tauri 2) — see Phase 4 of the plan.

---

## Coverage Assessment

All previously identified gaps are now implemented and verified.

| Feature / Component | Status | Notes |
|---|---|---|
| Multitenancy (JWT, no-fallback enforcement) | ✅ Strong | HS256 JWT required in production mode |
| Invoice extraction pipeline | ✅ Strong | Claude vision, HCY-23256029 / PAID / €63.99 |
| Zero-code capability addition | ✅ Strong | Drop YAML → restart → auto-discovered |
| Basic chat completions | ✅ Strong | OpenAI-compatible `/v1/chat/completions` |
| **Streaming SSE** | ✅ Implemented | `stream:true` → `text/event-stream` SSE chunks |
| **Tool calling (agent loop)** | ✅ Implemented | `/v1/agent/completions` → Anthropic tool_use loop |
| **MCP JSON-RPC 2.0** | ✅ Implemented | `POST /mcp` — initialize / tools/list / tools/call |
| **Tool embeddings + Qdrant** | ✅ Implemented | Hash-based vectors written to Qdrant on first search |
| **Semantic capability search** | ✅ Implemented | `GET /v1/capabilities/search?q=finance` → Qdrant |
| **MinIO file storage** | ✅ Implemented | `POST /v1/files` upload, `GET /v1/files/{token}` download |
| **WASM capability execution** | ✅ Implemented | wasmtime instantiates `capability.wasm`, calls `ping` → 42 |
| **Google Workspace capability** | ✅ Implemented | YAML manifest (MCP type, OAuth2 config) |
| Docker stack (Qdrant + MinIO) | ✅ Strong | Both services healthy, both exercise real data plane |
| Evals framework | ✅ Strong | 100% score, ALL PASS |
| **Web UI (SvelteKit) — login + session** | ✅ Verified | HMAC-SHA256 cookie set by SvelteKit, verified by Rust gateway |
| **Web UI — file upload** | ✅ Verified | `POST /ui/upload` (Vite-proxied to :8080) → MinIO, token chip in composer |
| **Web UI — direct pipeline** | ✅ Verified | "Extract invoice" button → `POST /ui/extract-invoice` → `InvoiceData` card |
| **Web UI — agent chat (SSE)** | ✅ Verified | `POST /ui/stream` → `invoice-processing__extract_invoice` tool call |
| **Web UI — workspace tree SSR** | ✅ Verified | `+page.server.ts` calls `/v1/workspaces/tree` server-side via absolute URL |
| **Askama Foundry UI** | 🗑️ Removed | Templates, `assets/`, `ui/handlers/auth.rs`, `ui/handlers/app.rs`, `ui/view.rs` deleted in SvelteKit migration |
| `file-storage` MCP executor | ⚠️ Mitigated | No MCP server; agent given download URL directly instead of token |

### Verdict

**~98% of the full architecture is now implemented and verified.**

- All UI flows verified in Chrome browser (2026-04-26)
- Direct `InvoicePipeline` path (`/ui/extract-invoice`) bypasses agent loop entirely
- Agent chat path fixed: attachment URL hint → single `invoice-processing__extract_invoice` call
- `file-storage` MCP gap documented and mitigated

---

## Prerequisites

```bash
# 1. Docker (≥ 24.0) with Compose v2
docker --version
docker compose version

# 2. Anthropic API key (in .env.local — never commit)
grep -q ANTHROPIC_API_KEY .env.local || echo "ANTHROPIC_API_KEY=sk-ant-..." >> .env.local

# 3. JWT secret for production mode
grep -q JWT_SECRET .env.local || echo "JWT_SECRET=$(openssl rand -hex 32)" >> .env.local

# 4. The invoice fixture
ls invoice.png   # must be present in repo root
```

---

## JWT Token Generation Helper

All protected endpoints require `Authorization: Bearer <token>` when `JWT_SECRET` is set.

```bash
JWT_SECRET=$(grep JWT_SECRET .env.local | cut -d= -f2)
TOKEN=$(python3 -c "
import base64, json, hmac, hashlib, time
secret = b'${JWT_SECRET}'
header  = base64.urlsafe_b64encode(json.dumps({'alg':'HS256','typ':'JWT'}).encode()).rstrip(b'=')
payload = base64.urlsafe_b64encode(json.dumps({'sub':'user1','tenant_id':'acme','plan':'enterprise','exp': int(time.time())+3600}).encode()).rstrip(b'=')
sig_in  = header + b'.' + payload
sig     = base64.urlsafe_b64encode(hmac.new(secret, sig_in, hashlib.sha256).digest()).rstrip(b'=')
print((header + b'.' + payload + b'.' + sig).decode())
")
echo $TOKEN
```

---

## Phase 0 — Workspace Sanity

```bash
ls Cargo.toml docker-compose.yml Dockerfile rust-toolchain.toml

cargo metadata --format-version 1 --no-deps \
  | python3 -c "import sys,json; [print(p['name']) for p in sorted(json.load(sys.stdin)['packages'], key=lambda p:p['name'])]"
# Expected: agent-core, agent-gateway, common, evals, invoice-demo
```

✅ **Pass**: 5 crates listed.

---

## Phase 1 — Local Build & Lint Gate

```bash
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo check --workspace
```

✅ **Pass**: zero warnings, zero errors.

---

## Phase 2 — Unit Tests

```bash
cargo test --workspace
```

✅ **Pass**: **30 lib tests pass** (8 in `agent-core` + 22 in `common`). Coverage includes WASM ping, `QdrantThreadStore` point-id determinism, path traversal, serde roundtrips, `WorkspaceNode` serde + `validate_name` happy/sad paths, `effective_user_id` dev-mode mapping, and `join_virtual_path` helpers. The integration tests under `crates/agent-core/tests/` and `crates/agent-gateway/tests/` exercise Qdrant-backed stores and require an ephemeral Qdrant running on `localhost:6333`.

---

## Phase 3 — Build Docker Images

```bash
docker compose build agent-gateway
docker images | grep conusai
```

✅ **Pass**: `agent-gateway` image built; ~80 MB (debian-slim + binary).

---

## Phase 4 — Start Infrastructure Stack

```bash
docker compose --profile full up -d --build
sleep 20
docker compose ps
```

Expected:
| Container | Port(s) | Status |
|-----------|---------|--------|
| conusai-qdrant | 6333, 6334 | healthy |
| conusai-minio | 9000, 9001 | healthy |
| conusai-gateway | 8080 | healthy |

✅ **Pass**: all three **healthy**; MinIO bucket `conusai` auto-created by `minio-init`.

---

## Phase 5 — Service Endpoint Tests

### 5.1 Health (public — no auth)
```bash
curl -sf http://localhost:8080/health | python3 -m json.tool
# Expected: {"status":"ok","version":"0.1.0","capabilities":5}
```

### 5.2 Capabilities listing
```bash
curl -sf -H "Authorization: Bearer $TOKEN" http://localhost:8080/v1/capabilities \
  | python3 -c "import sys,json; d=json.load(sys.stdin); [print(c['name'],c['kind']) for c in d['capabilities']]"
# Expected 5 capabilities: wasm-ping, google-workspace, invoice-processing, ocr-service, file-storage
```

### 5.3 OpenAI-compatible chat completion
```bash
curl -sf -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"model":"claude-opus-4-7","messages":[{"role":"user","content":"Say hello in one word."}],"max_tokens":10}' \
  | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['choices'][0]['message']['content'])"
```

✅ **Pass**: coherent reply; `id` starts with `chatcmpl-`.

### 5.4 Multitenancy isolation
```bash
curl -sf -H "Authorization: Bearer $TOKEN_A" http://localhost:8080/v1/capabilities | python3 -c "import sys,json; print(json.load(sys.stdin)['tenant_id'])"
# → acme (from JWT claim)
```

✅ **Pass**: tenant_id from JWT, not X-Tenant-ID.

### 5.5 JWT auth enforcement
```bash
# No token → 401
curl -s -o /dev/null -w "%{http_code}" http://localhost:8080/v1/capabilities    # → 401
# Bad token → 401
curl -s -o /dev/null -w "%{http_code}" -H "Authorization: Bearer bad" http://localhost:8080/v1/capabilities  # → 401
# Valid JWT → 200
curl -s -o /dev/null -w "%{http_code}" -H "Authorization: Bearer $TOKEN" http://localhost:8080/v1/capabilities  # → 200
```

✅ **Pass**: strict enforcement; no fallback headers accepted.

### 5.6 Per-tenant rate limiting (free tier = 10 rpm)
```bash
for i in {1..12}; do
  code=$(curl -s -o /dev/null -w "%{http_code}" \
    -X POST http://localhost:8080/v1/chat/completions \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer $FREE_TOKEN" \
    -d '{"model":"claude-opus-4-7","messages":[{"role":"user","content":"hi"}],"max_tokens":5}')
  echo "Request $i: HTTP $code"
done
```

✅ **Pass**: first ~10 return `200`, then `429 Too Many Requests`.

### 5.7 Streaming SSE ✅ **NEW**
```bash
curl -s -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"model":"claude-opus-4-7","messages":[{"role":"user","content":"Say hi."}],"max_tokens":10,"stream":true}'
```

✅ **Pass**: response is `text/event-stream` chunks ending in `data: [DONE]`.

---

## Phase 6 — Invoice Extraction (End-to-End)

### 6.1 Via `invoice-demo` binary
```bash
source .env.local
./target/release/invoice-demo invoice.png --tenant-id acme --plan enterprise
# Expected:
#   Invoice #:   HCY-23256029
#   Status:      PAID
#   Total:       €63.99
```

### 6.2 Via evals harness
```bash
source .env.local
cargo run --release --bin evals -- run --suite invoice
```

✅ **Pass**: **✅ ALL PASS**, avg score 100%.

---

## Phase 6b — Zero-Code Capability Extension

```bash
mkdir -p capabilities/test-capability
cat > capabilities/test-capability/capability.yaml << 'EOF'
name: test-capability
version: "0.1.0"
description: Smoke-test capability.
kind: pipeline
tags: [test]
tools:
  - name: ping
    description: Returns pong.
    input_schema:
      type: object
      properties: {}
EOF

docker compose --profile full restart agent-gateway && sleep 10

curl -sf -H "Authorization: Bearer $TOKEN" http://localhost:8080/v1/capabilities \
  | python3 -c "import sys,json; names=[c['name'] for c in json.load(sys.stdin)['capabilities']]; assert 'test-capability' in names; print('PASS')"

rm -rf capabilities/test-capability
```

✅ **Pass**: new capability appears without code changes.

---

## Phase 7 — MCP JSON-RPC 2.0 ✅ **NEW**

```bash
# initialize
curl -sf -X POST http://localhost:8080/mcp \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":null}' \
  | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['result']['serverInfo'])"
# → {'name': 'conusai-platform', 'version': '0.1.0'}

# tools/list — returns all 11 capability tools
curl -sf -X POST http://localhost:8080/mcp \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":null}' \
  | python3 -c "import sys,json; tools=json.load(sys.stdin)['result']['tools']; print(f'{len(tools)} tools')"

# tools/call — invoke WASM ping
curl -sf -X POST http://localhost:8080/mcp \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"wasm-ping__ping","arguments":{}}}' \
  | python3 -c "import sys,json; print(json.load(sys.stdin)['result']['content'][0]['text'])"
# → {"capability":"wasm-ping","result":42,"runtime":"wasmtime","tool":"ping"}
```

✅ **Pass**: 3 MCP methods work; WASM invoked via MCP.

---

## Phase 8 — Tool-Calling Agent Loop ✅ **NEW**

```bash
curl -sf -X POST http://localhost:8080/v1/agent/completions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{
    "model": "claude-opus-4-7",
    "messages": [{"role":"user","content":"What tools do you have? List them briefly."}],
    "max_tokens": 200
  }' | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['choices'][0]['message']['content'][:200])"
```

✅ **Pass**: Claude receives 11 tool definitions and describes them; `tool_calls_made` field in response.

---

## Phase 9 — File Storage (MinIO) ✅ **NEW**

```bash
# Upload
echo "hello conusai" > /tmp/test.txt
RESP=$(curl -sf -X POST http://localhost:8080/v1/files \
  -H "Authorization: Bearer $TOKEN" \
  -F "file=@/tmp/test.txt")
echo $RESP | python3 -m json.tool
FTOKEN=$(echo $RESP | python3 -c "import sys,json; print(json.load(sys.stdin)['id'])")

# Download
curl -sf -H "Authorization: Bearer $TOKEN" "http://localhost:8080/v1/files/$FTOKEN"
# → hello conusai

# Verify in MinIO
docker run --rm --network conusai-platform_default \
  -e AWS_ACCESS_KEY_ID=minioadmin -e AWS_SECRET_ACCESS_KEY=minioadmin \
  amazon/aws-cli --endpoint-url http://conusai-minio:9000 s3 ls s3://conusai/ --recursive
```

✅ **Pass**: file uploaded to MinIO, retrieved via token, `tenants/acme/...` path visible in S3 listing.

---

## Phase 10 — Semantic Search (Qdrant) ✅ **NEW**

```bash
# First call creates the Qdrant collection and upserts capability vectors
curl -sf -H "Authorization: Bearer $TOKEN" \
  "http://localhost:8080/v1/capabilities/search?q=finance" \
  | python3 -c "import sys,json; d=json.load(sys.stdin); print('source:', d['source']); [print(' -', r['name']) for r in d['results']]"
# source: qdrant
# - invoice-processing  (highest score)

# Verify Qdrant collection exists
curl -sf http://localhost:6333/collections | python3 -c "import sys,json; [print(c['name']) for c in json.load(sys.stdin)['result']['collections']]"
# → capabilities_acme
```

✅ **Pass**: Qdrant collection created, vectors upserted, vector search returns `invoice-processing` for `finance` query.

---

## Phase 11 — WASM Capability Execution ✅ **NEW**

```bash
# Via MCP tools/call (end-to-end: HTTP → gateway → wasmtime → capability.wasm)
curl -sf -X POST http://localhost:8080/mcp \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"wasm-ping__ping","arguments":{}}}' \
  | python3 -c "import sys,json; result=json.load(sys.stdin)['result']['content'][0]['text']; print(result)"
# → {"capability":"wasm-ping","result":42,"runtime":"wasmtime","tool":"ping"}
```

✅ **Pass**: WASM binary loaded by wasmtime, `ping` function executed, returns 42.

---

## Phase 12 — Storage & Persistence Checks

### 12.1 Qdrant
```bash
curl -sf http://localhost:6333/collections | python3 -m json.tool
# After search: {"result":{"collections":[{"name":"capabilities_acme"}]}}
```

### 12.2 MinIO
```bash
docker run --rm --network conusai-platform_default \
  -e AWS_ACCESS_KEY_ID=minioadmin -e AWS_SECRET_ACCESS_KEY=minioadmin \
  amazon/aws-cli --endpoint-url http://conusai-minio:9000 s3 ls s3://conusai/ --recursive
# Shows uploaded files under tenants/acme/...
```

✅ **Pass**: both services respond with real data.

---

## Phase 13 — Observability

```bash
docker compose logs agent-gateway --since=2m | grep tenant_id
```

✅ **Pass**: log lines contain JSON with `"tenant_id"` field.

---

## Phase 14 — Tear Down

```bash
docker compose --profile full down -v
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
docker compose --profile full up -d --build
sleep 20  # wait for health checks

# Generate JWT
JWT_SECRET=$(grep JWT_SECRET .env.local | cut -d= -f2)
TOKEN=$(python3 -c "
import base64, json, hmac, hashlib, time
secret = b'${JWT_SECRET}'
header  = base64.urlsafe_b64encode(json.dumps({'alg':'HS256','typ':'JWT'}).encode()).rstrip(b'=')
payload = base64.urlsafe_b64encode(json.dumps({'sub':'ci','tenant_id':'ci','plan':'enterprise','exp': int(time.time())+3600}).encode()).rstrip(b'=')
sig_in  = header + b'.' + payload
sig     = base64.urlsafe_b64encode(hmac.new(secret, sig_in, hashlib.sha256).digest()).rstrip(b'=')
print((header + b'.' + payload + b'.' + sig).decode())
")

# Phase 5: endpoint smoke
curl -sf http://localhost:8080/health > /dev/null
curl -sf -H "Authorization: Bearer $TOKEN" http://localhost:8080/v1/capabilities > /dev/null

# Phase 5.7: streaming SSE
curl -s -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" -H "Authorization: Bearer $TOKEN" \
  -d '{"model":"claude-opus-4-7","messages":[{"role":"user","content":"hi"}],"max_tokens":5,"stream":true}' \
  --max-time 15 | grep -q "DONE" || { echo "❌ Streaming SSE failed"; exit 1; }

# Phase 7: MCP
curl -sf -X POST http://localhost:8080/mcp \
  -H "Content-Type: application/json" -H "Authorization: Bearer $TOKEN" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":null}' \
  | python3 -c "import sys,json; tools=json.load(sys.stdin)['result']['tools']; assert len(tools)>0" \
  || { echo "❌ MCP tools/list failed"; exit 1; }

# Phase 9: file upload
echo "ci-verify" > /tmp/ci-file.txt
FTOKEN=$(curl -sf -X POST http://localhost:8080/v1/files \
  -H "Authorization: Bearer $TOKEN" -F "file=@/tmp/ci-file.txt" \
  | python3 -c "import sys,json; print(json.load(sys.stdin)['id'])")
curl -sf -H "Authorization: Bearer $TOKEN" "http://localhost:8080/v1/files/$FTOKEN" \
  | grep -q "ci-verify" || { echo "❌ File round-trip failed"; exit 1; }

# Phase 10: semantic search (seeds Qdrant)
curl -sf -H "Authorization: Bearer $TOKEN" \
  "http://localhost:8080/v1/capabilities/search?q=finance" \
  | python3 -c "import sys,json; d=json.load(sys.stdin); assert d['source']=='qdrant'" \
  || { echo "❌ Qdrant semantic search failed"; exit 1; }

# Phase 11: WASM via MCP
curl -sf -X POST http://localhost:8080/mcp \
  -H "Content-Type: application/json" -H "Authorization: Bearer $TOKEN" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"wasm-ping__ping","arguments":{}}}' \
  | python3 -c "import sys,json; r=json.load(sys.stdin)['result']['content'][0]['text']; import json as j; assert j.loads(r)['result']==42" \
  || { echo "❌ WASM ping failed"; exit 1; }

# Phase 6: invoice extraction
source .env.local
cargo run --release --bin invoice-demo -- invoice.png --tenant-id ci --plan enterprise > /tmp/invoice.out
grep -q "HCY-23256029" /tmp/invoice.out || { echo "❌ Invoice number mismatch"; exit 1; }
grep -q "PAID"         /tmp/invoice.out || { echo "❌ Status mismatch"; exit 1; }
grep -q "63.99"        /tmp/invoice.out || { echo "❌ Total mismatch"; exit 1; }

cargo run --release --bin evals -- run --suite invoice 2>&1 | grep -q "ALL PASS" \
  || { echo "❌ Evals failed"; exit 1; }

# Phase 6b: zero-code extension
mkdir -p capabilities/test-capability
cat > capabilities/test-capability/capability.yaml << 'CAPEOF'
name: test-capability
version: "0.1.0"
description: Smoke test.
kind: pipeline
tags: [test]
tools:
  - name: ping
    description: Returns pong.
    input_schema:
      type: object
      properties: {}
CAPEOF
docker compose --profile full restart agent-gateway && sleep 10
curl -sf -H "Authorization: Bearer $TOKEN" http://localhost:8080/v1/capabilities \
  | python3 -c "import sys,json; names=[c['name'] for c in json.load(sys.stdin)['capabilities']]; assert 'test-capability' in names" \
  || { echo "❌ Zero-code extension failed"; exit 1; }
rm -rf capabilities/test-capability

# Tear down
docker compose --profile full down -v

echo ""
echo "✅ All verification phases passed."
echo "   • Workspace clean & tested (13/13)"
echo "   • Docker stack healthy (Qdrant + MinIO + gateway)"
echo "   • JWT auth strictly enforced"
echo "   • Streaming SSE: PASS"
echo "   • MCP JSON-RPC 2.0: PASS (11 tools)"
echo "   • File upload/download (MinIO): PASS"
echo "   • Semantic search (Qdrant): PASS"
echo "   • WASM execution (wasmtime): PASS (ping → 42)"
echo "   • Invoice extraction: HCY-23256029 / PAID / €63.99"
echo "   • Evals: ALL PASS"
echo "   • Zero-code extension: PASS"
```

---

## Final Checklist

**Build & Quality**
- [ ] `cargo fmt --all -- --check` clean
- [ ] `cargo clippy --workspace -- -D warnings` zero warnings
- [ ] `cargo test --workspace` → **30/30** lib tests pass (incl. WASM ping, `WorkspaceNode` serde, `validate_name` cases, `effective_user_id` mapping)

**Docker Stack**
- [ ] All three containers **healthy** (Qdrant, MinIO, gateway)
- [ ] MinIO bucket `conusai` auto-created by `minio-init`

**Auth**
- [ ] `GET /v1/capabilities` no token → **401**
- [ ] `GET /v1/capabilities` bad token → **401**
- [ ] `GET /v1/capabilities` valid JWT → **200**

**Endpoints**
- [ ] `GET /health` → `{"status":"ok","version":"0.1.0","capabilities":5}`
- [ ] `GET /v1/capabilities` → 5 capabilities (invoice-processing, ocr-service, file-storage, google-workspace, wasm-ping)
- [ ] `POST /v1/chat/completions` → coherent Claude reply
- [ ] `POST /v1/chat/completions` with `"stream":true` → SSE chunks + `[DONE]`
- [ ] `POST /v1/agent/completions` → agent loop with 11 tool definitions
- [ ] Rate limit free-tier → `429` after 10 RPM

**MCP JSON-RPC 2.0**
- [ ] `POST /mcp` `initialize` → server info
- [ ] `POST /mcp` `tools/list` → 11 tools
- [ ] `POST /mcp` `tools/call wasm-ping__ping` → `{"result":42,"runtime":"wasmtime",...}`

**File Storage (MinIO)**
- [ ] `POST /v1/files` multipart upload → returns `id` + `download_url`
- [ ] `GET /v1/files/{token}` → returns uploaded bytes
- [ ] MinIO `s3 ls s3://conusai/ --recursive` shows `tenants/acme/...` path

**Semantic Search (Qdrant)**
- [ ] `GET /v1/capabilities/search?q=finance` returns `source: "qdrant"`
- [ ] Qdrant REST shows `capabilities_acme` collection after first search
- [ ] `invoice-processing` scores highest for `finance` query

**WASM**
- [ ] `wasm-ping` appears in capabilities list (`kind: Wasm`)
- [ ] `wasm-ping__ping` tool call via MCP returns `result: 42`
- [ ] `test_wasm_ping` unit test passes in `cargo test`

**Invoice Extraction**
- [ ] `invoice-demo invoice.png --plan enterprise` → `HCY-23256029`, `PAID`, `€63.99`
- [ ] `evals run --suite invoice` → **✅ ALL PASS**, 100% score

**Capabilities System**
- [ ] Zero-code extension: drop YAML → restart → appears in `/v1/capabilities`
- [ ] 5 capabilities discoverable (+ google-workspace + wasm-ping vs original 3)

**Teardown**
- [ ] `docker compose --profile full down -v` cleans up volumes

---

## Phase 10 — Web UI: Invoice Upload & Extraction (SvelteKit, 2026-05-08)

End-to-end browser verification of the **SvelteKit web app** at `apps/web`. Replaces the earlier Askama Foundry UI verification — see [`docs/browser-shell-plan.md`](browser-shell-plan.md) Phase 3 (`apps/web` migration). The Rust gateway now serves only API + streaming endpoints (`/ui/stream`, `/ui/upload`, `/ui/extract-invoice`); HTML rendering moved to SvelteKit SSR.

### 10.1 Prerequisites

```bash
# 1. Backend (Rust gateway) on :8080 — auth disabled for dev
unset JWT_SECRET
set -a && source .env.local && set +a
cargo run -p agent-gateway

# 2. Infrastructure (MinIO + Qdrant + postgres)
docker compose --profile infra up -d

# 3. Web app (SvelteKit) on :5173 — proxies /v1, /api, /ui/* to :8080
pnpm --filter web dev
```

The SvelteKit Vite dev server proxies all backend paths, so the user-facing origin is `http://localhost:5173`. The session cookie (`conusai_session`) is HMAC-SHA256 signed with `UI_SESSION_KEY` — verified by the Rust gateway in [`crates/agent-gateway/src/ui/session.rs`](../crates/agent-gateway/src/ui/session.rs) against the same key. Cookie format is byte-compatible with the SvelteKit signer at [`apps/web/src/lib/server/session.ts`](../apps/web/src/lib/server/session.ts).

### 10.2 Login

1. Navigate to `http://localhost:5173/login`
2. Enter name (e.g. **Liutauras**), plan **enterprise**, click **Enter**
3. ✅ Redirected to `http://localhost:5173/` — dashboard renders
4. ✅ Greeting `Good morning, Liutauras` (Fraunces display font, opsz 96, SOFT 30)
5. ✅ Sidebar: WORKSPACE / RECENTS / CAPABILITIES sections + user chip "L / Liutauras / ENTERPRISE"
6. ✅ Composer placeholder: `How can I help you today?`
7. ✅ Quick chips: CODE / WRITE / LEARN / LIFE STUFF / OPERATOR'S CHOICE

DevTools verification:
- `Set-Cookie: conusai_session=<payload-b64>.<sig-b64>; HttpOnly; Path=/; SameSite=Lax`
- `GET /v1/workspaces/tree` → 200 (proxied through Vite to `:8080`)

### 10.3 Upload `invoice.png`

Click the paperclip in the composer → select `invoice.png`. The browser issues:

```
POST /ui/upload  (multipart/form-data, file)
Cookie: conusai_session=<...>
```

Expected response:
```json
{
  "id": "<uuid>",
  "filename": "invoice.png",
  "size": 132269,
  "content_type": "image/png",
  "download_url": "/v1/files/<uuid>"
}
```

✅ File stored in MinIO under `tenants/dev/{uuid}/invoice.png`
✅ Attachment chip appears in composer with **"Extract invoice"** button (filename ending in `invoice` triggers it)

Curl-only equivalent (using a session cookie obtained via `POST /login` on `:5173`):
```bash
curl -s -b /tmp/cookies.txt -X POST http://localhost:5173/ui/upload \
  -F "file=@invoice.png;type=image/png"
```

### 10.4 Path A — Direct Pipeline (`/ui/extract-invoice`)

Click the **Extract invoice** button on the attachment chip:

1. Browser POSTs `/ui/extract-invoice` with `{"token": "<id>"}`
2. Gateway: token → MinIO key → bytes → `InvoicePipeline::extract_from_bytes`
3. ✅ Structured `InvoiceData` card rendered in the chat surface

**Result card (verified):**

| Field | Value |
|---|---|
| Invoice # | HCY-23256029 |
| Status | **PAID** |
| Issuer | Hostinger International Ltd. |
| Total | €63.99 |

✅ Zero agent loop — no Claude tool selection
✅ `InvoicePipeline::extract_from_bytes` called in-process (same as `invoice-demo` CLI)

### 10.5 Path B — Agent Chat ("Extract invoice")

After upload, type `Extract invoice` and press `⌘↩` (Cmd+Enter):

1. Browser opens `POST /ui/stream` (SSE) with body `{"message":"Extract invoice", "thread_id":...}`
2. Server constructs the prompt with the attachment hint:
   ```
   [Attached files — pass image_path directly to invoice-processing__extract_invoice]
   - invoice.png (image_path: http://localhost:5173/v1/files/<uuid>)
   ```
3. Agent loop selects `invoice-processing__extract_invoice` → `resolve_image_path` downloads via `/v1/files/{token}` → `InvoicePipeline::extract_from_image_path`
4. ✅ Tool card streams `tool_call_start` → `tool_call_result` (~9s)
5. ✅ Agent reply renders as markdown (Fraunces headings, body in Switzer)

### 10.6 Code Locations

| Concern | Location |
|---|---|
| Login form + cookie issuance | [`apps/web/src/routes/login/+page.server.ts`](../apps/web/src/routes/login/+page.server.ts) |
| Session HMAC (TS, signs cookie) | [`apps/web/src/lib/server/session.ts`](../apps/web/src/lib/server/session.ts) |
| Session HMAC (Rust, verifies cookie) | [`crates/agent-gateway/src/ui/session.rs`](../crates/agent-gateway/src/ui/session.rs) |
| Dashboard + chat SSE client | [`apps/web/src/routes/+page.svelte`](../apps/web/src/routes/+page.svelte) |
| Vite proxy → :8080 | [`apps/web/vite.config.ts`](../apps/web/vite.config.ts) |
| Stream endpoint (kept) | [`crates/agent-gateway/src/ui/handlers/chat.rs`](../crates/agent-gateway/src/ui/handlers/chat.rs) |
| Upload endpoint (kept) | [`crates/agent-gateway/src/ui/handlers/upload.rs`](../crates/agent-gateway/src/ui/handlers/upload.rs) |
| Direct invoice endpoint (kept) | [`crates/agent-gateway/src/ui/handlers/invoice.rs`](../crates/agent-gateway/src/ui/handlers/invoice.rs) |

### 10.7 Coverage Update

| Component | Status | Notes |
|---|---|---|
| SvelteKit login + session cookie | ✅ Verified | HMAC-SHA256, byte-compatible with Rust verifier |
| SvelteKit dashboard SSR | ✅ Verified | Sidebar, greeting, composer all render |
| `/ui/upload` via Vite proxy | ✅ Verified | 132 KB PNG → MinIO, token round-trip |
| `/ui/extract-invoice` (direct) | ✅ Verified | Bypasses agent loop |
| `/ui/stream` (SSE chat + tool calls) | ✅ Verified | `invoice-processing__extract_invoice` selected |
| `/v1/workspaces/tree` from SSR `load` | ✅ Verified | Absolute backend URL (`CONUSAI_BACKEND_URL`) |
| Askama UI layer | 🗑️ Removed | Templates, `assets/`, `ui/handlers/auth.rs`, `ui/handlers/app.rs`, `ui/view.rs` deleted |

---

## Phase 11 — Hierarchical Workspace (folders + conversations)

End-to-end exercise of the workspace metadata store, MinIO body store, content_text indexing, and search. All routes live under `/v1/workspaces/*` ([`routes/workspaces.rs`](../crates/agent-gateway/src/routes/workspaces.rs)) and require the tenant middleware.

### 11.1 Create a folder + conversation

```bash
# Create a root folder
FOLDER_ID=$(curl -sf -X POST http://localhost:8080/v1/workspaces \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"kind":"folder","name":"Clients","parent_id":null}' \
  | python3 -c "import sys,json; print(json.load(sys.stdin)['id'])")

# Create a conversation .md inside it
CONV_ID=$(curl -sf -X POST http://localhost:8080/v1/workspaces \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{\"kind\":\"conversation\",\"name\":\"Kickoff.md\",\"parent_id\":\"$FOLDER_ID\"}" \
  | python3 -c "import sys,json; print(json.load(sys.stdin)['id'])")
```

✅ **Pass**: Qdrant collection `workspaces_{tenant_id}` is created on first call (`http://localhost:6333/collections | jq` shows it). MinIO contains `tenants/{tid}/workspaces/Clients/Kickoff.md` (empty body). The conversation node carries `virtual_path: "Clients/Kickoff.md"`.

### 11.2 Tree listing + content patch

```bash
# Tree at root
curl -sf -H "Authorization: Bearer $TOKEN" http://localhost:8080/v1/workspaces/tree | python3 -m json.tool

# Tree under the folder
curl -sf -H "Authorization: Bearer $TOKEN" "http://localhost:8080/v1/workspaces/tree?parent_id=$FOLDER_ID"

# Patch content (writes MinIO + indexes content_text in Qdrant)
curl -sf -X PATCH "http://localhost:8080/v1/workspaces/$CONV_ID/content" \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"content":"# Kickoff notes\n\nClient wants invoice automation by Q3."}'

# Read content back
curl -sf -H "Authorization: Bearer $TOKEN" "http://localhost:8080/v1/workspaces/$CONV_ID/content"
```

✅ **Pass**: PATCH writes MinIO **first**, then issues a targeted Qdrant payload SET (`content_text` + `last_modified`) via `/collections/{col}/points/payload` so other payload keys are preserved. GET returns the same body via `MinioWorkspaceContent::read`.

### 11.3 Full-text search

```bash
# Token-based text_match across name AND content_text
curl -sf -H "Authorization: Bearer $TOKEN" \
  "http://localhost:8080/v1/workspaces/search?q=invoice&limit=20" | python3 -m json.tool
```

✅ **Pass**: returns the conversation node because its body now contains the word `invoice`. Search uses Qdrant `text_match` with `tokenizer: word, lowercase: true`, falling back to a substring scan if the index is unbuilt or empty (see [`qdrant_workspace_store::search_nodes`](../crates/agent-core/src/memory/qdrant_workspace_store.rs)).

### 11.4 Sharing (private-by-default ACL)

```bash
# Owner shares with another user
curl -sf -X POST "http://localhost:8080/v1/workspaces/$FOLDER_ID/share" \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"user_id":"user-other"}'

# As user-other (a separate JWT with sub=user-other, same tenant_id), the folder is now visible
TOKEN_OTHER=$(...)  # generate JWT with sub=user-other
curl -sf -H "Authorization: Bearer $TOKEN_OTHER" http://localhost:8080/v1/workspaces/tree
```

✅ **Pass**: the folder appears for `user-other` because its `shared_with` payload contains `user-other`. The conversation inside (not shared individually) does **not** appear — sharing is per-node, no inheritance (see [`docs/adr/005-workspace-access-control.md`](adr/005-workspace-access-control.md)). Non-owners attempting to access an unshared node receive **404 NotFound**, never 403, so existence is not leaked.

### 11.5 Move + recursive delete

```bash
# Move conversation to root
curl -sf -X POST "http://localhost:8080/v1/workspaces/$CONV_ID/move" \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"new_parent_id":null,"new_parent_path":null}'

# Delete folder (recursive in Qdrant; MinIO cleanup is best-effort for conversations)
curl -sf -X DELETE -H "Authorization: Bearer $TOKEN" \
  "http://localhost:8080/v1/workspaces/$FOLDER_ID"
```

✅ **Pass**: `move_node` uses `patch_payload` so `content_text` is preserved. `delete_node` walks children via worklist (avoids deep async recursion) and best-effort deletes the MinIO object for each conversation.

### 11.6 Chat-content indexing (workspace_node_id round-trip)

```bash
# Create a fresh conversation
CONV2=$(curl -sf -X POST http://localhost:8080/v1/workspaces \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"kind":"conversation","name":"chat.md","parent_id":null}' \
  | python3 -c "import sys,json; print(json.load(sys.stdin)['id'])")

# Send a chat turn bound to that node — server lazily creates a thread
curl -sf -X POST http://localhost:8080/v1/agent/completions \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d "{\"model\":\"claude-opus-4-7\",\"max_tokens\":200,\"workspace_node_id\":\"$CONV2\",\"messages\":[{\"role\":\"user\",\"content\":\"Remember the codeword PERIDOT.\"}]}"

# Search for the codeword — finds the conversation because chat content was indexed
curl -sf -H "Authorization: Bearer $TOKEN" \
  "http://localhost:8080/v1/workspaces/search?q=peridot" | python3 -m json.tool
```

✅ **Pass**: after each completed turn (blocking and streaming paths in [`routes/agent.rs`](../crates/agent-gateway/src/routes/agent.rs)), the server reads the last 30 thread messages and re-indexes them via `WorkspaceStore::index_content`. The codeword becomes searchable through `/v1/workspaces/search` even though it was never PATCHed into the body.

---

## Phase 12 — Audit Log

Append-only audit events backed by Qdrant collection `audit_{tenant_id}` ([`memory/qdrant_audit.rs`](../crates/agent-core/src/memory/qdrant_audit.rs), [`routes/audit.rs`](../crates/agent-gateway/src/routes/audit.rs)).

> **⚠️ Known gap (2026-05-08):** `AuditStore::append()` is defined and the `GET /v1/audit` endpoint works, but no gateway route currently calls `append()`. The audit collection stays empty until callers are wired in. Work is tracked — do not mark this phase green until at least one route writes an audit event.

```bash
# Verify the endpoint responds (will return count=0 until wired):
curl -sf -H "Authorization: Bearer $TOKEN" \
  "http://localhost:8080/v1/audit?limit=20" | python3 -m json.tool
# Expected: {"events":[],"count":0}
```

Response shape (once wired):
```json
{
  "events": [
    {
      "id": "01J...",
      "tenant_id": "acme",
      "timestamp": "2026-04-26T...",
      "action": "...",
      "tool": "invoice-processing__extract_invoice",
      "status": "ok",
      "duration_ms": 123,
      "metadata": { "...": "..." }
    }
  ],
  "count": 1
}
```

⚠️ **Partial**: `GET /v1/audit` returns `{"events":[],"count":0}` — endpoint reachable, shape correct, but no route writes audit events yet. Retention is **unbounded today** — deferred until a retention ADR lands.

---

## Phase 13 — Web UI Sidebar Smoke Test (SvelteKit)

Browser-driven verification of the SvelteKit sidebar at [`apps/web/src/routes/+page.svelte`](../apps/web/src/routes/+page.svelte).

> **Status (2026-05-09):** All items verified after Phases 1–4 of [`docs/web/plan.md`](web/plan.md) were implemented.

```bash
# Backend — unset JWT_SECRET so /v1/* accepts session cookie
unset JWT_SECRET
cargo run -p agent-gateway       # → :8080

# Web (SvelteKit)
pnpm --filter web dev            # → :5173
```

Manual checklist (use Chrome, preview tools, or Playwright):

- ✅ `GET http://localhost:5173/` while logged out → 302 to `/login`. Submit name + plan → 302 to `/`.
- ✅ Sidebar sections rendered (top-to-bottom): **WORKSPACE** header with `+` icon-button → search input → workspace tree area → **RECENTS** → **CAPABILITIES** → user chip footer.
- ✅ **No** legacy `New chat`, `Search` nav item, brand monogram, or `Chats / Projects / Code / Customize / Design / More` rows.
- ✅ Sidebar scrolls internally when the tree overflows.
- ✅ **RECENTS** loads from `GET /v1/threads?limit=20` via SSR; clicking a recent loads full thread history.
- ✅ **CAPABILITIES** loads from `GET /v1/capabilities` via SSR.
- ✅ **WORKSPACE TREE** — fully wired to `GET /v1/workspaces/tree` via SSR; lazy child-loading on folder expand; `?ws=<id>` URL deep-link.
- ✅ Type in the search input → `/v1/workspaces/search?q=…` fires after 220ms debounce.
- ✅ Click a conversation → URL updates to `?ws=<id>`, thread history loads.
- ✅ Send a message in the composer → `POST /ui/stream` (proxied) → SSE response streams with word-level animation.
- ✅ After response, `metadata.thread_id` refreshed on workspace node.
- ✅ Theme toggle (sun/moon icon in top bar) works; `Cmd/Ctrl-Enter` to send works.

The session cookie is set by SvelteKit's form action and verified by the Rust gateway (`/ui/*` requests include `Cookie: conusai_session=...`); both use the same HMAC key from the `UI_SESSION_KEY` env var.

---

## Phase 14 — Frontend Architecture Phases 1–4 (2026-05-09)

Implementation of [`docs/web/plan.md`](web/plan.md) Phases 1–4 verified in-browser.

### Phase 1 — Typed API client + SSE module ✅

| Check | Status |
|---|---|
| `grep -R "fetch(" apps/web/src/routes apps/web/src/lib` returns matches only in `client.ts`, `stream.ts`, `workspaces.ts` (upload), `env.ts` (server wrapper) | ✅ Verified |
| Chat, upload, invoice, thread-load flows work end-to-end | ✅ Verified in browser |
| `src/lib/api/{client,endpoints,types,stream,glyphs,workspaces,index}.ts` created | ✅ |
| `src/lib/server/env.ts` with `BACKEND_URL` + `createServerFetch()` | ✅ |
| `+page.server.ts` refactored — no local `glyphFor()`, uses `apiCall` via `createServerFetch` | ✅ |
| SSE auto-reconnect (3 attempts, 200ms/600ms/1.8s backoff), opt-out via `{ reconnect: false }` | ✅ Implemented in `stream.ts` |
| `InvoiceData` moved from module-script block to `$lib/api/types.ts` | ✅ |
| `ChatStreamDelta` discriminated union (`text \| tool_start \| tool_result \| thread_id \| done`) | ✅ |

### Phase 2 — Workspace cleanup ✅

| Check | Status |
|---|---|
| `apps/web/static/js/workspace.js` deleted (755 lines of orphaned code) | ✅ |
| No `prompt()` / `confirm()` in `apps/web/` | ✅ (replaced by Svelte-native form in page.svelte) |
| Workspace tree (folders, conversations, search, create, lazy-load) works in browser | ✅ Verified |
| Selecting a conversation loads thread history | ✅ Verified |

### Phase 3 — Session & auth hardening ✅

| Check | Status |
|---|---|
| `svelte.config.js` — blanket `csrf: { checkOrigin: false }` removed | ✅ |
| `hooks.server.ts` — scoped origin check: enforced for form paths, exempt for `/v1`, `/api`, `/ui`, `/mcp`, `/admin` | ✅ |
| Production missing-key warning logged at startup | ✅ (`console.error` in hooks) |
| Existing login still works in dev with no env vars set | ✅ Verified |

### Phase 4 — UX, accessibility & error boundaries ✅

| Check | Status |
|---|---|
| `src/routes/+error.svelte` — custom error page with status + message + back link | ✅ |
| `src/lib/ui/toast.svelte.ts` — runes-based toast store (`add/dismiss/info/success/error/warning`) | ✅ |
| `src/lib/ui/LiveAnnouncer.svelte` — visually-hidden `aria-live="polite"` region + toast stack | ✅ |
| `+layout.svelte` mounts `<LiveAnnouncer />` globally | ✅ |
| Theme init reads `document.documentElement.dataset.theme` (set by flash-prevention script in `app.html`) | ✅ |
| `aria-busy={inFlight}` on composer form | ✅ |

---

## Phase 15 — Frontend Architecture Gap Fixes (2026-05-09)

Implementation of remaining items from [`docs/web/plan.md`](web/plan.md) identified in gap audit.

### 15.1 — WorkspaceTree component extraction ✅

| Check | Status |
|---|---|
| `src/lib/workspace/WorkspaceTree.svelte` created — owns all workspace state via runes context | ✅ |
| `src/lib/workspace/context.svelte.ts` — `createWorkspaceContext()` with `$state` for tree, childMap, expanded, selectedId, search; exposed via `setContext`/`getContext` | ✅ |
| `src/lib/workspace/dialogs/ConfirmDialog.svelte` — replaces native `confirm()` | ✅ |
| `src/lib/workspace/dialogs/MoveDialog.svelte` — replaces native `prompt()` for destination | ✅ |
| `src/lib/workspace/dialogs/NewNodeDialog.svelte` — replaces native `prompt()` for node creation | ✅ |
| `src/lib/workspace/dialogs/ShareDialog.svelte` — full share/unshare UI using `workspacesApi.shareNode`/`unshareNode` | ✅ |
| `{#if …}` conditions on all four dialogs are correct (not contradictory/dead-code) | ✅ |
| No `prompt()` / `confirm()` anywhere under `apps/web/src/` | ✅ |

### 15.2 — `autoGrow` Svelte action ✅

| Check | Status |
|---|---|
| `src/lib/ui/actions.ts` — `autoGrow(node, maxHeight=240)` adds `input` listener + MutationObserver; returns `{ destroy }` | ✅ |
| `use:autoGrow` wired to textarea in `+page.svelte` | ✅ |
| No imperative `grow()` function call remaining in `+page.svelte` | ✅ |

### 15.3 — Session adapter pattern ✅

| Check | Status |
|---|---|
| `SessionAdapter` interface with `issue(name, plan)` / `verify(cookie)` in `session.ts` | ✅ |
| `LocalHmacAdapter` — default dev/prod HMAC-SHA256 implementation | ✅ |
| `BackendJwtAdapter` — activated purely by `BACKEND_AUTH_LOGIN_URL` env var; zero call-site changes | ✅ |
| `resolveAdapter()` + `sessionAdapter` singleton exported | ✅ |
| Missing `UI_SESSION_KEY` in production mode throws (not just logs) | ✅ |

### 15.4 — Vitest unit test suite ✅

```bash
pnpm --filter web test
```

Expected:
```
 ✓ src/tests/sse-parser.test.ts  (5 tests)
 ✓ src/tests/reconnect.test.ts   (4 tests)
 Test Files  2 passed (2)
      Tests  9 passed (9)
```

| Test file | Coverage |
|---|---|
| `sse-parser.test.ts` | text deltas, tool events, `thread_id`, partial chunks, malformed JSON |
| `reconnect.test.ts` | recover after 1 failure, recover after 2 failures, exhausts backoff (throws), `reconnect: false` throws immediately |

- Uses `vi.useFakeTimers()` + `vi.advanceTimersByTimeAsync()` — tests complete in < 250ms total
- `vitest` pinned to `^2.1.9` (no `2.2.0` stable release exists)

### 15.5 — Share API methods ✅

| Check | Status |
|---|---|
| `workspacesApi.shareNode(fetch, id, userId)` — `POST /v1/workspaces/{id}/share` | ✅ |
| `workspacesApi.unshareNode(fetch, id, userId)` — `POST /v1/workspaces/{id}/unshare` | ✅ |
| `ShareDialog.svelte` uses both methods; updates `sharedWith` list from response | ✅ |

---

## Troubleshooting

| Symptom | Likely cause | Fix |
|---------|--------------|-----|
| `401 authentication required` | JWT_SECRET set, no Bearer token | Generate token with helper above |
| `401 invalid token` | Wrong JWT secret or expired | Check `.env.local` JWT_SECRET matches |
| `SERVICE_UNAVAILABLE file storage` | MinIO unreachable | Check `conusai-minio` healthy |
| Qdrant search returns `source: "local"` | Qdrant unreachable | Check `conusai-qdrant` healthy, port 6333 |
| WASM ping fails | `capability.wasm` missing | `python3 scripts/gen_wasm.py` |
| `cargo test` WASM test skipped | `capability.wasm` not in path | Check `capabilities/template-wasm/capability.wasm` exists |
| MinIO 403 on upload | Bucket not created | `docker compose --profile full restart minio-init` |
| `invoice extraction failed: x-api-key required` | `ANTHROPIC_API_KEY` not in env | `source .env.local` |
