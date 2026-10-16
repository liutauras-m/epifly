# Deploying epifly to ConusAI Cloud

epifly runs on ConusAI Cloud as **one compose project**: the repository's
[`docker-compose.yml`](../docker-compose.yml) with eight services, plus four
linked managed services that hold all durable state outside the gateway's own
volume. This page is the complete procedure: what to create, every variable,
the first-start steps for Zitadel and Lago, how to verify, how to run the same
stack locally, and what changes when the platform's managed Identity and
Billing services arrive.

| Service | Image / build | Public | Role |
|---|---|---|---|
| `web` | [`apps/web/Dockerfile`](../apps/web/Dockerfile) | port 3000, **primary** | SvelteKit backend-for-frontend: OIDC login, encrypted session cookie, same-origin proxy to the gateway |
| `gateway` | [`apps/backend/Dockerfile`](../apps/backend/Dockerfile) | port 8080 | `agent-gateway`: chat, agents, capabilities, workspaces, billing API, `/admin` |
| `current-time` | [`services/current-time/Dockerfile`](../services/current-time/Dockerfile) | no | Example MCP capability; registers itself with the gateway at start |
| `zitadel` | `ghcr.io/zitadel/zitadel:v4.19.5` | port 8080 | Identity provider (OIDC), built-in login v1 |
| `lago-migrate` | `getlago/api:v1.55.0` | no (job) | One-shot Lago database migration |
| `lago-api` | `getlago/api:v1.55.0` | port 3000 | Lago billing API |
| `lago-worker` | `getlago/api:v1.55.0` | no | Lago background jobs |
| `lago-clock` | `getlago/api:v1.55.0` | no | Lago scheduler |

All three built images run as the non-root uid `10001`. The gateway's only
volume is `/data` (redb database and the embedding-model cache); Lago keeps
`/app/storage` on the `lago_storage` volume.

## 1. Prerequisites

- A ConusAI Cloud organization with room for the project (sizes in
  [`conusai.json`](../conusai.json): gateway `large`; web, `current-time`,
  `lago-clock` `small`; zitadel, `lago-api`, `lago-worker` `medium`).
- Access to the Git repository and its branch `conusai-cloud`.
- An Anthropic API key (`ANTHROPIC_API_KEY`).
- `openssl` and `bash` on the machine where you generate secrets
  ([`scripts/generate-env.sh`](../scripts/generate-env.sh)).
- `curl` for the Lago webhook step and the verification checklist.

## 2. Managed services to create and link

Create these four managed services in the same organization and link each one
to the project. Each link injects variables into every deployment of the
project; the compose file interpolates them, so none of them is set by hand.

| Managed service | Injected variables used | Used by | Why |
|---|---|---|---|
| Postgres | `POSTGRES_URL`, `POSTGRES_HOST`, `POSTGRES_PORT`, `POSTGRES_USER`, `POSTGRES_PASSWORD` | `zitadel` (host, port, user, password), Lago (`DATABASE_URL` = `POSTGRES_URL`) | Zitadel's event store and Lago's database. Zitadel's `start-from-init` connects with the linked user as admin and creates its own database `zitadel` and user `zitadel` (password `ZITADEL_DB_PASSWORD`). The gateway does not use Postgres. |
| Redis | `REDIS_URL` | Lago (`REDIS_URL`, `LAGO_REDIS_CACHE_URL`) | Lago's Sidekiq queue and cache. |
| Qdrant | `QDRANT_GRPC_URL`, `QDRANT_API_KEY` | `gateway` | Vector store for workspace search and the semantic capability router. |
| S3 | `S3_BUCKET`, `AWS_ENDPOINT_URL`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY` | `gateway` | Tenant workspaces, uploads and artifacts (RustFS). The gateway's per-tenant S3 credentials are stored in redb, encrypted with `RUSTFS_IAM_ENC_KEY`. |

The platform also injects `CONUSAI_API_URL` and `CONUSAI_API_TOKEN` (the
deployment token) into every deployment. The compose file uses
`CONUSAI_API_TOKEN` as the bearer of the gateway's OTLP export
(`OTEL_EXPORTER_OTLP_HEADERS=Authorization=Bearer%20${CONUSAI_API_TOKEN}`).

## 3. Project settings

| Setting | Value |
|---|---|
| Preset | `docker-compose` |
| Compose file | `docker-compose.yml` (named in `conusai.json` → `compose.file`) |
| Branch | `conusai-cloud` |
| Public services | `web:3000` (primary), `gateway:8080`, `zitadel:8080`, `lago-api:3000` |

The public services, their ports, sizes and the required variable names come
from [`conusai.json`](../conusai.json); the console shows them read-only. The
platform generates one hostname per public service (pattern
`<service>-<env-host>.<apps_base>`, for example
`zitadel-epifly-production.stage.stage.cloud.conusai.com`); a custom domain
can replace any of them. Three of those hostnames are fed back into the
compose file as project variables (`WEB_HOST`, `AUTH_HOST`, `BILLING_HOST`,
section 4). The gateway's hostname is needed only by clients that call the
gateway directly: the native shell (`VITE_GATEWAY_URL`) and API-key clients.

## 4. Project variables

Set every variable on the project **scoped to the environment** (production,
stage, …): each environment has its own Zitadel instance, Lago organization
and encryption keys, and a value shared between environments would couple
them. Generate the secrets once with

```bash
scripts/generate-env.sh > /tmp/epifly.env && chmod 600 /tmp/epifly.env
```

and copy the generated values into the console (then delete the file). The
script writes localhost values for the three hostnames; replace those. Column
"Gen." marks what the script generates.

**Never regenerate** `ZITADEL_MASTERKEY`, `RUSTFS_IAM_ENC_KEY`,
`LAGO_ENCRYPTION_PRIMARY_KEY`, `LAGO_ENCRYPTION_DETERMINISTIC_KEY`,
`LAGO_ENCRYPTION_KEY_DERIVATION_SALT`, `LAGO_SECRET_KEY_BASE` or
`LAGO_RSA_PRIVATE_KEY` for an environment that has data: they encrypt or sign
stored data (Zitadel's keys and secrets, the S3 IAM credentials the gateway
stores per tenant, Lago's encrypted columns, sessions and webhook signatures),
and a new value makes that data unreadable.

### 4.1 Project variables (set by you)

| Variable | Service(s) | Required | Gen. | How to generate / where from | Notes |
|---|---|---|---|---|---|
| `WEB_HOST` | web, gateway | yes | localhost | The `web` service's hostname (generated or custom), no scheme | Becomes `ORIGIN` (web) and `WEB_ORIGIN` (gateway CORS) as `https://${WEB_HOST}`. The Zitadel redirect URIs are `https://${WEB_HOST}/auth/callback` and `/auth/signed-out`. |
| `AUTH_HOST` | zitadel, web, gateway | yes | localhost | The `zitadel` service's hostname | `ZITADEL_EXTERNALDOMAIN`; issuer `https://${AUTH_HOST}` for web and gateway. Changing it later breaks existing tokens and the Zitadel instance's domain. |
| `BILLING_HOST` | Lago | yes | localhost | The `lago-api` service's hostname | `LAGO_API_URL` and `LAGO_FRONT_URL` = `https://${BILLING_HOST}`. The gateway itself calls Lago internally at `http://lago-api:3000`. |
| `ANTHROPIC_API_KEY` | gateway | yes | no | Anthropic console | The only LLM provider key the gateway requires. |
| `PLATFORM_ADMIN_TOKEN` | gateway, current-time | yes | yes | `openssl rand -base64 32` | At least 32 characters. Bearer for `GET /metrics` and `POST /admin/capabilities/register` (capability self-registration). |
| `RUSTFS_IAM_ENC_KEY` | gateway | yes | yes | `openssl rand -base64 32` | Base64 of exactly 32 bytes; encrypts the per-tenant S3 credentials. **Never regenerate.** |
| `AUTH_SESSION_KEY` | web | yes | yes | `openssl rand -base64 32` | Base64 of exactly 32 bytes; AES-256-GCM key of the `__Host-epifly_session` cookie. Rotating it signs every user out (nothing else is lost). |
| `ZITADEL_MASTERKEY` | zitadel | yes | yes | `openssl rand -hex 16` (32 characters) | Exactly 32 characters. Encrypts Zitadel's keys and secrets. **Never regenerate.** |
| `ZITADEL_DB_PASSWORD` | zitadel | yes | yes | `openssl rand -hex 24` | Password of the `zitadel` database user that `start-from-init` creates. |
| `ZITADEL_ADMIN_USERNAME` | zitadel | yes | yes (`admin`) | Choose | First-instance human admin of the `epifly` organization; read only at the first start. |
| `ZITADEL_ADMIN_PASSWORD` | zitadel | yes | yes | Generated; must meet Zitadel's password policy | Change is forced at first login (`PASSWORDCHANGEREQUIRED=true`). Read only at the first start. |
| `ZITADEL_AUDIENCE` | gateway | yes | no | Zitadel project id (section 5.1) | Required `aud` of access tokens. Before the project exists, set a placeholder such as `pending` so the gateway starts. |
| `ZITADEL_WEB_CLIENT_ID` | web | yes | no | Client id of the Zitadel web application (section 5.1) | Placeholder `pending` before the app exists. |
| `LAGO_SECRET_KEY_BASE` | Lago | yes | yes | `openssl rand -hex 64` | Rails secret. **Never regenerate.** |
| `LAGO_ENCRYPTION_PRIMARY_KEY` | Lago | yes | yes | `openssl rand -hex 16` | Active Record encryption. **Never regenerate.** |
| `LAGO_ENCRYPTION_DETERMINISTIC_KEY` | Lago | yes | yes | `openssl rand -hex 16` | **Never regenerate.** |
| `LAGO_ENCRYPTION_KEY_DERIVATION_SALT` | Lago | yes | yes | `openssl rand -hex 16` | **Never regenerate.** |
| `LAGO_RSA_PRIVATE_KEY` | Lago | yes | yes | `openssl genrsa 2048 \| base64 -w0` | Base64 of a PEM private key. **Never regenerate.** |
| `LAGO_API_KEY` | gateway | no | no | Lago organization API key (section 5.2) | Billing is **on** once set: the gateway then requires `LAGO_API_URL` (set in compose) and `LAGO_WEBHOOK_SECRET`, resolves each tenant's plan from Lago and seeds the plan catalog. Empty: every tenant is on the Free plan. |
| `LAGO_WEBHOOK_SECRET` | gateway | with `LAGO_API_KEY` | yes (placeholder) | Lago organization HMAC key (section 5.2) | Verifies `X-Lago-Signature` (Base64 HMAC-SHA256 of the body) on `POST /v1/billing/webhooks`. The generated value only fills the slot; replace it with Lago's key. |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | gateway | no | no | The platform's OTLP base, `<platform>/otel` (for example `https://stage.cloud.conusai.com/otel`) | The gateway appends `/v1/traces`, `/v1/metrics`, `/v1/logs` and exports over OTLP HTTP/protobuf. Empty: no export (logs on stdout, `/metrics` only). |

### 4.2 Optional gateway variables (not in `conusai.json`)

The gateway reads its whole environment once at start
([`config.rs`](../apps/backend/crates/agent-gateway/src/config.rs), with
[`common/src/env.rs`](../apps/backend/crates/common/src/env.rs)); a missing or
malformed value stops it with exit code 2 and one line naming every offending
variable. To use one of these, add it to the gateway's `environment` in
`docker-compose.yml`.

| Variable | Default | Meaning |
|---|---|---|
| `API_KEYS` | none | Machine-client keys: `<blake3-hex>:<tenant>:<label>,…`. Hash a key with `echo -n "$KEY" \| docker compose run --rm -T gateway --hash-api-key` (reads stdin). `<tenant>` is a Zitadel organization id. |
| `ZITADEL_INTROSPECTION_CLIENT_ID`, `ZITADEL_INTROSPECTION_CLIENT_SECRET` | none | Both or neither. Enables introspection of opaque access tokens (a Zitadel API application with Basic auth). Without them only JWT access tokens are accepted. |
| `LLM_ALIAS_SMART`, `LLM_ALIAS_OPUS`, `LLM_ALIAS_HAIKU`, `LLM_ALIAS_FAST`, `LLM_ALIAS_CHEAP` | `claude-sonnet-4-6`, `claude-opus-4-7`, `claude-haiku-4-5` (last three) | Anthropic model id behind each alias (`state.rs::build_llm_registry`). |
| `OPENAI_API_KEY` | none | Only for the video-transcription job (Whisper). |
| `RUSTFS_NOTIFICATION_WEBHOOK_URL`, `RUSTFS_WEBHOOK_SECRET` | none | Both or neither. Mounts `POST /internal/rustfs/events` for event-driven indexing. |
| `RUSTFS_BOOTSTRAP`, `RUSTFS_SSE`, `RUSTFS_VERSIONING`, `RUSTFS_QUOTAS` | `on` | `on`/`off` switches for storage bootstrap, server-side encryption, versioning, quotas. |
| `RUSTFS_PRESIGN_TTL_SECS` | `900` (max 3600) | Presigned URL lifetime. |
| `RUSTFS_PUBLIC_BASE_URL` | none | Public base for presigned URLs when the S3 endpoint is internal. |
| `RUSTFS_KEY_ROTATION_DAYS`, `AUDIT_RETENTION_DAYS` | `90` | Key rotation and audit retention. |
| `EMBEDDING_LOCAL_MODEL`, `EMBEDDING_MAX_BATCH` | `multilingual-e5-large`, `256` | Local embedding model. |
| `SEMANTIC_ROUTER_TOP_K`, `CONUSAI_ROUTER_MIN_CONFIDENCE`, `CONUSAI_MAX_TOOLS_PER_TURN`, `CONUSAI_MAX_INVOKES_PER_TURN` | `20`, built-in | Capability routing limits. |
| `CONUSAI_PLAN_CATALOG_PATH` | built-in | Plan catalog file seeded into Lago. |
| `BILLING_RETURN_URL` | `/account/billing` | Upgrade link in quota errors. |

Set by the image or the compose file, not by you: `PORT=8080`,
`CONUSAI_CAPABILITIES_DIR=/app/capabilities`, `REDB_PATH=/data/epifly.redb`,
`EMBEDDING_CACHE_DIR=/data/embeddings` (Dockerfile); `LAGO_API_URL`,
`CONUSAI_MCP_ALLOWED_HOSTS=current-time`, `OTEL_SERVICE_NAME`,
`OTEL_EXPORTER_OTLP_HEADERS`, `RUST_LOG` (compose). The web server's
environment (`ORIGIN`, `GATEWAY_URL`, `ZITADEL_ISSUER`, `ZITADEL_WEB_CLIENT_ID`,
optional `ZITADEL_WEB_CLIENT_SECRET` for a confidential client,
`AUTH_SESSION_KEY`, `BODY_SIZE_LIMIT`) is validated the same way at start by
[`apps/web/src/lib/server/config.ts`](../apps/web/src/lib/server/config.ts).

### 4.3 Linked-service variables

Injected by the links of section 2; never set them on the project:
`POSTGRES_URL`, `POSTGRES_HOST`, `POSTGRES_PORT`, `POSTGRES_USER`,
`POSTGRES_PASSWORD`, `REDIS_URL`, `QDRANT_GRPC_URL`, `QDRANT_API_KEY`,
`S3_BUCKET`, `AWS_ENDPOINT_URL`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`,
`CONUSAI_API_URL`, `CONUSAI_API_TOKEN`.

## 5. First start

1. Link the four managed services, set every variable of section 4.1 with
   `ZITADEL_AUDIENCE=pending` and `ZITADEL_WEB_CLIENT_ID=pending`, and leave
   `LAGO_API_KEY` empty.
2. Deploy once. Then read the generated hostnames of `web`, `zitadel` and
   `lago-api` from the environment's domains (or attach custom domains), set
   `WEB_HOST`, `AUTH_HOST` and `BILLING_HOST`, and **redeploy**. Zitadel
   writes its external domain into the instance at the first start, so if
   `AUTH_HOST` was wrong on that first start, the Zitadel database must be
   recreated (or the instance domain added in Zitadel) before logins work.
3. Wait until `zitadel` and `lago-api` are healthy (`lago-migrate` exits 0
   first).

### 5.1 Zitadel

Open `https://<AUTH_HOST>/ui/console` and sign in as the first-instance admin
(`ZITADEL_ADMIN_USERNAME`; Zitadel may expect the full login name
`<username>@epifly.<AUTH_HOST>`), then set a new password.

1. **Project.** Create a project (for example `epifly`). In its settings turn
   on **Assert Roles on Authentication** so the roles claim
   `urn:zitadel:iam:org:project:roles` is in the tokens. The project id
   (Resource Id) is the value of `ZITADEL_AUDIENCE`.
2. **Roles.** Add the role keys `super_admin` and `admin`. `super_admin`
   grants `/admin/*` on the gateway; `admin` maps to the gateway's `Admin`
   role, which no route requires today. Grant `super_admin` to the operator's user (Authorizations).
3. **Web application.** New application, type **Web**, authentication method
   **PKCE** (user agent, no secret):
   - Redirect URI: `https://<WEB_HOST>/auth/callback`
   - Post-logout redirect URI: `https://<WEB_HOST>/auth/signed-out`
   - Token settings: auth token type **JWT**; grant types authorization code
     and **refresh token**.

   Its client id is `ZITADEL_WEB_CLIENT_ID`. (A confidential client also works:
   set `ZITADEL_WEB_CLIENT_SECRET` on `web` in the compose file.)
4. **Native application** (desktop shell). New application, type **Native**,
   PKCE, redirect URI `http://127.0.0.1/callback`, JWT access tokens, refresh
   token grant. The shell listens on a random loopback port
   (`http://127.0.0.1:<port>/callback`); Zitadel's native apps accept any port
   on a loopback redirect. Its client id goes into the shell build as
   `VITE_ZITADEL_NATIVE_CLIENT_ID`.
5. **Tenant claim.** The gateway takes the tenant from the access token claim
   `urn:zitadel:iam:user:resourceowner:id` and rejects tokens without it.
   Zitadel adds that claim when the client requests the scope
   `urn:zitadel:iam:user:resourceowner`.
6. Set `ZITADEL_AUDIENCE` and `ZITADEL_WEB_CLIENT_ID` on the project.

Each Zitadel organization is one epifly tenant; users of the `epifly`
organization share its workspace and plan.

### 5.2 Lago

The compose file runs Lago's API, worker and clock but not Lago's web front,
and sign-up is disabled (`LAGO_DISABLE_SIGNUP=true`). Create the organization
and its API key with Lago's seeding task, in a shell of the running `lago-api`
container:

```bash
LAGO_CREATE_ORG=true LAGO_ORG_NAME=epifly \
LAGO_ORG_USER_EMAIL=<operator email> LAGO_ORG_USER_PASSWORD=<password> \
LAGO_ORG_API_KEY="$(openssl rand -hex 32)" \
bundle exec rails signup:seed_organization
```

`LAGO_ORG_API_KEY` is the value of `LAGO_API_KEY`. The organization's HMAC
key, which Lago generates, is the value of `LAGO_WEBHOOK_SECRET`:

```bash
bundle exec rails runner 'print Organization.find_by!(name: "epifly").hmac_key'
```

Register the gateway's webhook with HMAC signing (the gateway refuses
JWT-signed webhooks); `lago-api` reaches the gateway on the project network:

```bash
curl -sf -X POST "https://<BILLING_HOST>/api/v1/webhook_endpoints" \
  -H "Authorization: Bearer $LAGO_API_KEY" -H 'Content-Type: application/json' \
  -d '{"webhook_endpoint":{"webhook_url":"http://gateway:8080/v1/billing/webhooks","signature_algo":"hmac"}}'
```

Set `LAGO_API_KEY` and `LAGO_WEBHOOK_SECRET` on the project.

### 5.3 Redeploy

Redeploy the environment. On start the gateway seeds the plan catalog into
Lago (`Lago plan catalog seeded` in its log). A tenant's plan is the plan code
of its active Lago subscription (`pro` → Pro, `team`/`enterprise` →
Enterprise, anything else or none → Free), cached for five minutes and
invalidated by subscription webhooks. The plan decides which model aliases a
client may request (Free: `haiku`, `fast`, `cheap`; Pro adds `smart`;
Enterprise adds `opus`).

## 6. Verification checklist

| Check | How | Expected |
|---|---|---|
| TLS | `curl -sI https://<WEB_HOST>/healthz` | Valid certificate; `strict-transport-security` header |
| Web health | `curl -s https://<WEB_HOST>/healthz` | `ok` |
| Gateway health | `curl -s https://<gateway host>/health` | 200 |
| Gateway auth | `curl -s -o /dev/null -w '%{http_code}' https://<gateway host>/v1/capabilities` | `401` with `WWW-Authenticate: Bearer` |
| Metrics | `curl -s -H "Authorization: Bearer $PLATFORM_ADMIN_TOKEN" https://<gateway host>/metrics` | Prometheus text (401 without the token) |
| Zitadel discovery | `curl -s https://<AUTH_HOST>/.well-known/openid-configuration` | `issuer` is exactly `https://<AUTH_HOST>` |
| Login | Open `https://<WEB_HOST>/` | Redirect to Zitadel, sign in, back on the app; cookie `__Host-epifly_session` set |
| Chat turn | Send a message in the app | A streamed answer |
| Upload | Upload a file in the app | It appears in the workspace tree and can be downloaded |
| Logout | Sign out | Lands on `/auth/signed-out`; `/` redirects to login again |
| Traces | Platform console → Observe | Traces, metrics and logs of service `epifly-gateway` |
| Billing (when on) | Open `/account/billing` in the app | The seeded plans |

## 7. Local run

```bash
scripts/generate-env.sh > .env && chmod 600 .env
# set ANTHROPIC_API_KEY, and ZITADEL_AUDIENCE / ZITADEL_WEB_CLIENT_ID (placeholder "pending" first)
just up        # docker compose -p epifly -f docker-compose.yml -f docker-compose.local.yml up --build -d
just logs gateway
just config    # render the merged file; fails when a variable is missing
just down      # stop, volumes kept
```

[`docker-compose.local.yml`](../docker-compose.local.yml) adds Postgres 18,
Valkey (Redis), Qdrant and RustFS as containers, plain-HTTP Zitadel, and
loopback host ports: web `127.0.0.1:3000`, gateway `127.0.0.1:8080`,
Zitadel `127.0.0.1:8085`, Lago API `127.0.0.1:3001`, RustFS `127.0.0.1:9000`.
OTLP export is off. The Zitadel steps of section 5.1 are the same with
`http://localhost:3000/auth/callback` and `http://localhost:3000/auth/signed-out`.
`just web-dev` runs the web app on the host with hot reload on port 5173.

## 8. Troubleshooting

| Symptom | Cause and fix |
|---|---|
| Gateway exits immediately with code 2 | Its log line `agent-gateway: environment contract not satisfied; missing: …; invalid: …` names every variable to fix. `ZITADEL_AUDIENCE` must not be empty (use `pending` before the Zitadel project exists). |
| `web` never starts | It waits for a healthy gateway (`depends_on`); fix the gateway first. A `[config] …` error in the web log names its own missing variable. |
| Login loops or `invalid or expired token` (401) on every API call | Access tokens are opaque (set the app's token type to JWT, or configure introspection), the audience is wrong (`ZITADEL_AUDIENCE` must be the project id), or the token has no `urn:zitadel:iam:user:resourceowner:id` claim. The gateway logs the reason at `warn` (`bearer token rejected`). |
| 503 `identity provider unavailable` | The gateway cannot fetch `https://<AUTH_HOST>/.well-known/openid-configuration` or the JWKS, or the discovered issuer differs from `ZITADEL_ISSUER`. |
| Zitadel shows "instance not found" | The request host is not the instance's domain: `AUTH_HOST` changed after the first start. |
| Redirect URI error at Zitadel | The web app's redirect URIs must equal `https://<WEB_HOST>/auth/callback` and `/auth/signed-out` exactly. |
| `/admin` returns 403 | The user lacks the `super_admin` project role, or the project does not assert roles. |
| Every tenant stays on Free | `LAGO_API_KEY` is empty, Lago is unreachable (`plan lookup failed` in the gateway log), or the subscription's plan code is not `pro`/`team`/`enterprise`. |
| Lago webhooks rejected | The endpoint uses JWT signing, or `LAGO_WEBHOOK_SECRET` is not the organization's HMAC key. |
| `lago-api` never healthy | `lago-migrate` failed: check its log; the linked Postgres and Redis must be reachable. |
| No traces in Observe | `OTEL_EXPORTER_OTLP_ENDPOINT` is empty or is not the `/otel` base; the export authenticates with the injected `CONUSAI_API_TOKEN`. |

## 9. Next: managed Identity and Billing

ConusAI Cloud is adding Identity (Zitadel) and Billing (Lago) as managed
services (conusai-cloud plan `managed-identity-billing`). Its item **E1**
moves epifly onto them: the `zitadel` service and the four `lago-*` services
leave `docker-compose.yml`, together with their variables (`AUTH_HOST`,
`BILLING_HOST`, `ZITADEL_*`, `LAGO_*` secrets); the project links Identity and
Billing next to Postgres, Redis, Qdrant and S3; the platform injects `OIDC_*`
(issuer, audience, web and native client ids) and `LAGO_*`; and Lago webhooks
are verified with RS256 instead of HMAC. The deployment then has three
services: `gateway`, `web`, `current-time`. This page describes the
compose-file setup until that change lands.
