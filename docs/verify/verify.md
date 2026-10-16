# ConusAI Platform — Docker Verification Plan

Status (2026-10-16): rewritten for the ConusAI Cloud layout. The previous plan drove the
stack through `start.sh` / `start-verify.sh`, HS256 tokens signed with `JWT_SECRET`,
`X-Tenant-ID` headers and a Jaeger/collector profile; all of that is gone. This plan
verifies the stack as `just up` runs it. Deployment: [deploy-conusai-cloud.md](../deploy-conusai-cloud.md).
Companions: [verify-web.md](verify-web.md), [verify-ios.md](verify-ios.md).

## What runs

`just up` = `docker compose -p epifly -f docker-compose.yml -f docker-compose.local.yml up --build -d`.

| Service | Source | Local port |
|---|---|---|
| `gateway` | `apps/backend/Dockerfile` (agent-gateway, uid 10001, volume `/data` for redb and the embedding model cache) | `127.0.0.1:8080` |
| `web` | `apps/web/Dockerfile` (SvelteKit BFF) | `127.0.0.1:3000` |
| `current-time` | `services/current-time` (self-registering remote MCP service) | — |
| `zitadel` | `ghcr.io/zitadel/zitadel:v4.19.5`, built-in login v1 | `127.0.0.1:8085` |
| `lago-migrate`, `lago-api`, `lago-worker`, `lago-clock` | `getlago/api:v1.55.0` | `lago-api` on `127.0.0.1:3001` |
| `postgres`, `redis`, `qdrant`, `rustfs` | `docker-compose.local.yml` only (managed services on ConusAI Cloud) | `rustfs` on `127.0.0.1:9000` |

Telemetry stays local (`OTEL_EXPORTER_OTLP_ENDPOINT` is empty in the local overlay): JSON
logs on stdout (`just logs gateway`) and Prometheus text at `/metrics`.

---

## Prerequisites

```bash
docker compose version            # Compose v2
just --version
openssl version
```

An Anthropic API key for `ANTHROPIC_API_KEY`. The invoice fixture is `invoice.png` at the
repository root.

---

## Phase 0 — Local gates

```bash
just verify        # clippy, tenant-path and capability-storage lints, UI token check, lint, tests, OpenAPI types drift
cargo xtask capabilities lint
```

---

## Phase 1 — Environment and start

```bash
scripts/generate-env.sh > .env && chmod 600 .env   # then set ANTHROPIC_API_KEY
just config                                        # merged compose renders, every variable set
just up
just logs gateway
```

The gateway validates its whole environment at start
(`apps/backend/crates/agent-gateway/src/config.rs`) and exits with code 2 and one line
listing every missing or invalid variable. On a fresh `.env` it reports
`ZITADEL_AUDIENCE`, and `web` (which waits for a healthy gateway) does not start:

1. Sign in to Zitadel at `http://localhost:8085` with `ZITADEL_ADMIN_USERNAME` /
   `ZITADEL_ADMIN_PASSWORD` from `.env` (a password change is required).
2. Create the project, web application and roles as in
   [deploy-conusai-cloud.md](../deploy-conusai-cloud.md), using
   `http://localhost:3000/auth/callback` and `http://localhost:3000/auth/signed-out`.
3. Set `ZITADEL_AUDIENCE` (project id) and `ZITADEL_WEB_CLIENT_ID` in `.env`, then
   `just up` again.

Billing stays off locally until `LAGO_API_KEY` is set; every tenant is then on the Free
plan.

Known issue: the local overlay sets `ZITADEL_ISSUER=http://localhost:8085` for gateway and
web. Inside those containers `localhost` is the container itself, so OIDC discovery and
JWKS fetches fail and bearer tokens are answered with `503 identity provider unavailable`
until the issuer is reachable under the same URL from the host and from the containers.

✅ **Pass**: `docker compose -p epifly ps` shows `gateway`, `web`, `zitadel`, `lago-api`
healthy and `lago-migrate` exited 0.

---

## Phase 2 — A bearer token

Every protected route takes `Authorization: Bearer <token>`, where the token is a Zitadel
access token or an API key from `API_KEYS`. Nothing else authenticates: no cookies, no
tenant header.

**Zitadel service user** (for curl). In the Zitadel console, in the `epifly`
organization: create a service user with access token type JWT, generate a client
secret, and grant it the project (add the `super_admin` role for Phase 5). Then:

```bash
PROJECT_ID=<ZITADEL_AUDIENCE>
TOKEN=$(curl -s -u "$CLIENT_ID:$CLIENT_SECRET" http://localhost:8085/oauth/v2/token \
  -d grant_type=client_credentials \
  --data-urlencode "scope=openid urn:zitadel:iam:org:project:id:${PROJECT_ID}:aud urn:zitadel:iam:user:resourceowner" \
  | jq -r .access_token)
```

The gateway accepts it when it is an RS256 JWT with `iss` = `ZITADEL_ISSUER`, `aud`
containing `ZITADEL_AUDIENCE`, a valid `exp`, and the organization claim
`urn:zitadel:iam:user:resourceowner:id`, which becomes the tenant. Roles come from
`urn:zitadel:iam:org:project:roles` (`super_admin`, `admin`).

**API key** (machine clients). Hash a random key with the gateway binary and configure
`API_KEYS=<blake3-hex>:<tenant>:<label>`:

```bash
KEY=$(openssl rand -hex 32)
printf '%s\n' "$KEY" | docker compose -p epifly -f docker-compose.yml -f docker-compose.local.yml \
  run --rm --no-deps -T gateway --hash-api-key
```

`docker-compose.yml` does not pass `API_KEYS` to the gateway, so a key only works where
the variable is added to the gateway's environment.

---

## Phase 3 — Public endpoints

```bash
curl -s http://localhost:8080/health | jq            # status ok, capabilities N, embeddings ok, router ok
curl -s http://localhost:8080/healthz/embeddings | jq  # model + dims once the model is loaded
curl -s http://localhost:8080/openapi.json | jq '.info'
curl -s -o /dev/null -w '%{http_code}\n' http://localhost:8080/metrics   # 401
curl -s -H "Authorization: Bearer $PLATFORM_ADMIN_TOKEN" http://localhost:8080/metrics | head
curl -s http://localhost:3000/healthz                 # ok
```

`/health` answers `503` with `"status":"degraded"` while the embedding model is still
loading (first start downloads it into `/data/embeddings`). The OpenAPI document is also
printed by `agent-gateway --dump-openapi`; the route table by `--dump-routes`. There is no
Swagger UI.

---

## Phase 4 — Authenticated endpoints

```bash
H="Authorization: Bearer $TOKEN"
G=http://localhost:8080

# No token / bad token → 401 with WWW-Authenticate: Bearer
curl -s -o /dev/null -w '%{http_code}\n' $G/v1/capabilities
curl -s -o /dev/null -w '%{http_code}\n' -H 'Authorization: Bearer nope' $G/v1/capabilities

# Capabilities and semantic search
curl -s -H "$H" $G/v1/capabilities | jq '.capabilities | length'
curl -s -H "$H" "$G/v1/capabilities/search?q=extract+invoice" | jq

# Chat (OpenAI-shaped); the model must be on the plan's allowlist, default when omitted
curl -s -H "$H" -H 'Content-Type: application/json' $G/v1/chat/completions \
  -d '{"messages":[{"role":"user","content":"Say STREAM_TEST_OK"}]}' | jq '.choices[0].message.content'
curl -s -o /dev/null -w '%{http_code}\n' -H "$H" -H 'Content-Type: application/json' $G/v1/chat/completions \
  -d '{"model":"opus","messages":[{"role":"user","content":"hi"}]}'          # 403 on Free

# Streaming
curl -sN -H "$H" -H 'Content-Type: application/json' $G/v1/chat/completions \
  -d '{"stream":true,"messages":[{"role":"user","content":"Count to 3"}]}'

# MCP: tools offered to this tenant (names are <capability with dots as _>__<tool>)
curl -s -H "$H" -H 'Content-Type: application/json' $G/mcp \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | jq '.result.tools[].name'

# Upload through the gateway (multipart; the first file field is stored)
curl -s -H "$H" -F file=@invoice.png $G/ui/upload | jq

# Workspace, threads, audit
curl -s -H "$H" -H 'Content-Type: application/json' $G/v1/workspaces \
  -d '{"kind":"folder","name":"verify"}' | jq
curl -s -H "$H" $G/v1/workspaces/tree | jq
curl -s -H "$H" $G/v1/threads | jq
curl -s -H "$H" $G/v1/audit | jq
```

Plan tiers and their model allowlists (`apps/backend/crates/agent-core/src/context/tenant.rs`):
Free `haiku`, `fast`, `cheap`; Pro adds `smart`; Enterprise adds `opus`. A tenant's plan
comes from its Lago subscription (`pro` → Pro, `team`/`enterprise` → Enterprise, anything
else or none → Free). Free tenants are rate-limited to 10 requests per minute on
`/v1/capabilities` and `/mcp`.

Presigned URLs (`POST /v1/files/upload-url`, `GET /v1/files/download-url`,
`/v1/uploads/*`, `/v1/workspaces/{id}/presign-*`) are signed for `AWS_ENDPOINT_URL`; locally
that is `http://rustfs:9000`, which only resolves inside the compose network unless
`RUSTFS_PUBLIC_BASE_URL` is set for the gateway.

---

## Phase 5 — Super-admin and platform routes

```bash
# super_admin role → 200; any other caller → 403
curl -s -H "$H" $G/admin/capabilities | jq 'length'
curl -s -H "$H" $G/admin/jobs | jq
curl -s -X POST -H "$H" $G/admin/capabilities/reload | jq

# Platform token registration (what current-time does at start)
REG='{"capability_id":"verify.echo","namespace":"verify","version":"1","kind":"remote_mcp",
 "endpoint":"http://current-time:8082/mcp","global":true,
 "tools":[{"name":"echo","description":"verification","input_schema":{"type":"object"}}]}'
curl -s -o /dev/null -w '%{http_code}\n' -X POST $G/admin/capabilities/register \
  -H 'Content-Type: application/json' -d "$REG"                              # 401 without the token
curl -s -X POST $G/admin/capabilities/register -H "Authorization: Bearer $PLATFORM_ADMIN_TOKEN" \
  -H 'Content-Type: application/json' -d "$REG"                              # 201; a second call → 409
```

---

## Phase 6 — Storage and vectors

```bash
# RustFS: objects of the tenant (shared bucket layout tenants/<org id>/workspaces/…,
# or bucket ws-<org id> after the tenant-bucket-migration job)
set -a; . ./.env; set +a
aws --endpoint-url http://127.0.0.1:9000 s3 ls "s3://$S3_BUCKET" --recursive | head
aws --endpoint-url http://127.0.0.1:9000 s3 ls        # per-tenant ws-* buckets

# Qdrant collections (API key from .env)
docker compose -p epifly -f docker-compose.yml -f docker-compose.local.yml exec gateway \
  sh -c 'curl -s -H "api-key: $QDRANT_API_KEY" http://qdrant:6333/collections'
```

---

## Phase 7 — Invoice extraction (evals harness)

```bash
GATEWAY_URL=http://localhost:8080 EVAL_BEARER_TOKEN=$TOKEN \
  cargo run -p evals -- run --suite invoice
```

✅ **Pass**: the invoice suite scores the fixture (HCY-23256029, PAID, €63.99).

---

## Phase 8 — Tear down

```bash
just down                     # volumes kept
```

---

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| Gateway exits with code 2 | Environment contract not satisfied | The log line names every missing/invalid variable |
| `401 invalid or expired token` | Wrong audience, issuer mismatch, expired, opaque token without introspection, or no organization claim | Check the token's `iss`, `aud`, `exp` and `urn:zitadel:iam:user:resourceowner:id`; opaque tokens need `ZITADEL_INTROSPECTION_CLIENT_ID`/`_SECRET` |
| `503 identity provider unavailable` | Gateway cannot fetch `ZITADEL_ISSUER/.well-known/openid-configuration` | See the known issue in Phase 1 |
| `403` on `/admin/*` | Caller lacks the `super_admin` project role | Grant the role; enable role assertion on the Zitadel project |
| `403` on chat with a `model` | Model not on the plan's allowlist | Omit `model` or pick an allowed alias |
| `tools/list` has no `current-time` tool | Registration refused | `just logs current-time`. The endpoint host must be in `CONUSAI_MCP_ALLOWED_HOSTS`, and the manifest may carry only the fields `/admin/capabilities/register` accepts, with a non-empty `tenant_scope` or `"global": true` (see [capability-authoring-guide.md](../capability-authoring-guide.md)) |
| `/health` `503` with `embeddings: fail` | Embedding model still downloading | Wait; check `just logs gateway` |
