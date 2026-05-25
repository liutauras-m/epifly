# Conusai Platform — Dokploy Deployment

Production deployment for **Epifly** at
`https://epifly.beta.test.cloud.conusai.com` with supporting services at
`*.epifly.beta.test.cloud.conusai.com`, using [Dokploy](https://dokploy.com)
+ Traefik on the shared `dokploy-network`.

## Architecture

Each folder below is registered as an independent **Compose** application
inside the same Dokploy **Project**. Splitting them means infra never
restarts when only the gateway or web is redeployed, and Dokploy can show
per-service health.

| Folder | Dokploy app name | Purpose | Public hostname |
|---|---|---|---|
| [infra/](infra/) | `conusai-infra` | Postgres, Redis, Qdrant, RustFS, Zitadel, Lago | `auth.`, `billing.`, `s3.`, `s3-console.` |
| [gateway/](gateway/) | `conusai-gateway` | Rust agent-gateway (HTTP + WS API) | `api.` |
| [capabilities/](capabilities/) | `conusai-capabilities` | Self-registering MCP services (current-time, …) | internal only |
| [web/](web/) | `conusai-web` | SvelteKit web app — also the remote target for Tauri shells | bare apex |
| [observability/](observability/) | `conusai-observability` | Jaeger + OTel collector | `traces.` |

Hostnames (with `APP_DOMAIN=epifly.beta.test.cloud.conusai.com`):

| Service | URL |
|---|---|
| Web (Epifly) | `https://epifly.beta.test.cloud.conusai.com` |
| Agent gateway | `https://api.epifly.beta.test.cloud.conusai.com` |
| Zitadel | `https://auth.epifly.beta.test.cloud.conusai.com` |
| Lago billing | `https://billing.epifly.beta.test.cloud.conusai.com` |
| RustFS S3 API | `https://s3.epifly.beta.test.cloud.conusai.com` |
| RustFS console | `https://s3-console.epifly.beta.test.cloud.conusai.com` |
| Jaeger | `https://traces.epifly.beta.test.cloud.conusai.com` |

All services share the cookie domain `.epifly.beta.test.cloud.conusai.com`
so a single Zitadel SSO session covers the web app + gateway + Lago portal.

> **Tauri desktop / iOS / Android shells** are *not* deployed via Dokploy.
> They are native binaries built in CI and distributed via the App Store,
> Play Store, or GitHub Releases. They point their `frontendDist` /
> `devUrl` at `https://epifly.beta.test.cloud.conusai.com`, which is the
> `conusai-web` service in this folder.

## Networking

All services join the external Docker network **`dokploy-network`**
(created automatically by Dokploy). Cross-stack communication uses the
fixed `hostname:` set on each service — for example the gateway reaches
Postgres at `conusai-postgres:5432` regardless of which compose project
they live in.

Public traffic is terminated by **Traefik** on `:443`
(`entrypoints=websecure`, `certresolver=letsencrypt`). Internal-only
services (Postgres, Redis, Qdrant, RustFS S3 API, current-time) have **no
Traefik labels and no host ports** — they're reachable only inside
`dokploy-network`.

## Volumes

Persistent volumes are declared `external: true` so they survive
recreation. Create them once via the Dokploy UI (Project → Volumes) **or**
let the first deploy create them and back them up via Dokploy's built-in
Volume Backups. The named volumes used are:

- `conusai_postgres_data`
- `conusai_qdrant_data`
- `conusai_rustfs_data`
- `conusai_redb_data`

## Secrets

Copy [`.env.example`](.env.example) into Dokploy's **Shared Environment**
(Project → Settings → Environment) so every compose app inherits the same
values. **Rotate every `changeme_*` placeholder before the first deploy.**

Sensitive values to set:

- `POSTGRES_PASSWORD`
- `ZITADEL_MASTERKEY` (exactly 32 chars)
- `LAGO_SECRET_KEY_BASE`, `LAGO_ENCRYPTION_*`, `LAGO_RSA_PRIVATE_KEY`
- `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY` (RustFS root creds)
- `UI_SESSION_KEY` (≥32 bytes)
- `PLATFORM_ADMIN_TOKEN`
- `RUSTFS_IAM_ENC_KEY`, `RUSTFS_WEBHOOK_SECRET`
- `STRIPE_SECRET_KEY` (if billing is live)
- LLM provider keys consumed by the gateway (`OPENAI_API_KEY`, etc.)

## Deploy order

1. **Create DNS**: easiest is one wildcard A record
   `*.epifly.beta.test.cloud.conusai.com → <dokploy host>` plus an apex A
   record for `epifly.beta.test.cloud.conusai.com` itself. Traefik then
   issues one Let's Encrypt cert per hostname automatically.
2. **Create the project** in Dokploy and add Shared Environment.
3. Deploy **`conusai-infra`** → wait for all healthchecks green
   (Zitadel takes ~60 s on first boot to seed its schema).
4. Visit `https://auth.epifly.beta.test.cloud.conusai.com`, complete Zitadel
   initial-admin setup, create the Epifly project + OIDC application,
   and record the issuer URL + client ID into Shared Environment as
   `ZITADEL_ISSUER` / `ZITADEL_CLIENT_ID`.
5. Visit `https://billing.epifly.beta.test.cloud.conusai.com`, complete
   Lago onboarding, generate the API key, store as `LAGO_API_KEY`.
6. Deploy **`conusai-gateway`**.
7. Deploy **`conusai-capabilities`**.
8. Deploy **`conusai-web`**.
9. (Optional) Deploy **`conusai-observability`**.

## Updating

Each Dokploy app has its own Git source / build trigger. Recommended:

- **infra** → manual deploy only (immutable image tags).
- **gateway / capabilities / web** → auto-deploy on `main` push.

Traefik picks up the new container labels on rolling restart with zero
downtime as long as healthchecks pass.
