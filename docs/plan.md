# ConusAI Auth + Tenancy — Detailed Implementation Plan v0.8

> Source: `docs/tasks/auth.md` v0.6 (2026-05-09) reconciled against the current
> code on `main`, then refined by the v0.8 review (2026-05-09). This is the
> **authoritative, code-grounded** plan.
> All effort estimates and high-level phasing match v0.6. Where v0.6 was vague
> or misaligned with the repo, this document gives the exact files, types,
> diffs, and acceptance gates.
>
> **v0.8 changelog** (vs v0.7):
> 1. `secrecy = "0.10"` for `JwtIssuer.secret` + provider creds (no `Debug` leaks).
> 2. Shared `common::jwt::JwtSecret::from_env()` — single source of truth for issuer + middleware.
> 3. `#[tracing::instrument]` + Prometheus counters on auth service entry points.
> 4. `#[non_exhaustive]` `AuthError` with proper `source` chains via `From` impls.
> 5. Provider registry stays as `HashMap` (no premature factory abstraction).
> 6. Frontend client renamed `GatewayClient` / `apiFetch` (was `BackendJwtAdapter`/`gatewayFetch`).
> 7. `AuthConfig::load` falls back to `CONUSAI_` prefix for transition (e.g. `JWT_SECRET`).
> 8. New round-trip integration test: `JwtIssuer::issue` → real `mw/tenant.rs` decode.
>
> **Effort**: 22–27 AI-hours (~145k tokens). +4h vs v0.7 baseline for the 8 refinements.

---

## 0. Code-reality reconciliation (what changed vs v0.6)

A full read of the workspace turned up the following deltas from the v0.6 doc.
They drive the corrections below; nothing in v0.6's *intent* changes.

| Area | v0.6 assumption | Reality on `main` | Action |
|---|---|---|---|
| Workspace manifest | `[workspace.members]` table | [Cargo.toml](Cargo.toml#L1-L8) uses `[workspace] members = [...]` array | Append `"crates/auth-core"` to the existing array, **not** a new table |
| `async-trait` | "already in workspace.deps" | Not in [Cargo.toml](Cargo.toml#L19-L97) `[workspace.dependencies]` | Add `async-trait = { version = "0.1" }` to workspace deps (currently only used inside `agent-core` indirectly) |
| `wiremock` | dev-dep, "already used" | Not present anywhere | Add as `[dev-dependencies]` of `auth-core` only |
| `figment` re-use | "existing pattern" | [common/src/config/mod.rs](crates/common/src/config/mod.rs#L1-L62) uses `Toml::file("config.toml") + Env::prefixed("CONUSAI_")` | Mirror this exact pattern in `auth-core::config` |
| `TenantClaims` shape | "already supports `tenant_id`" | [context/tenant.rs](crates/agent-core/src/context/tenant.rs#L102-L108) is `{ sub, tenant_id, plan, exp }` (HS256) | `JwtIssuer` MUST emit *exactly* this shape so [mw/tenant.rs](crates/agent-gateway/src/mw/tenant.rs#L40-L60) keeps working unchanged. Add fields only via additive, optional claims |
| JWT algorithm | RS256 *or* HS256 | [mw/tenant.rs](crates/agent-gateway/src/mw/tenant.rs#L43-L46) hard-codes HS256 + `JWT_SECRET` env | Phase A ships HS256 only; RS256/JWKS deferred to Phase D (post-ZITADEL prep) |
| `BackendJwtAdapter` (frontend) | "Phase 3 already prepared" | [apps/web/src/lib/server/session.ts](apps/web/src/lib/server/session.ts#L1-L60) is an HMAC cookie only; no JWT adapter exists | Phase C must **create** the adapter, not just "activate" it |
| Login UI | "buttons added" | [apps/web/src/routes/login/+page.svelte](apps/web/src/routes/login/+page.svelte#L1-L60) is a name+plan form (dev fake login) | Phase C adds provider buttons *next to* the dev form, gated by env |
| `crates/auth-core` | implied | Does not exist | Create from scratch |
| UI session ↔ tenant | "shared via cookie" | [ui/session.rs](crates/agent-gateway/src/ui/session.rs#L40-L55) reads tenant from `CONUSAI_UI_TENANT_ID` env (single tenant) | Phase B extends `SessionUser` to carry `tenant_id` + `email` after OAuth login |

**Conclusion:** v0.6's three-phase shape (A: `auth-core`, B: gateway wiring,
C: frontend + docs) is correct. The corrections above are tactical, not
architectural. We add **no new phases** beyond optional Phase D.

Effort revised: **A 13–15h · B 7–9h · C 3–4h · (D optional 4–6h)**
→ baseline 23–28 AI-hours (includes v0.8 refinements), still inside v0.6 band.

---

## 1. Phase A — `crates/auth-core` (library + tenant resolution)

Pure domain logic. **Zero** Axum/HTTP/SvelteKit coupling.

### 1.1 Workspace manifest diffs

[Cargo.toml](Cargo.toml) — append to the `members` array:

```toml
[workspace]
resolver = "3"
members = [
    "crates/common",
    "crates/agent-core",
    "crates/agent-gateway",
    "crates/auth-core",      # ← NEW
    "crates/invoice-demo",
    "evals",
]
```

Append to `[workspace.dependencies]`:

```toml
# OAuth / OIDC
oauth2          = { version = "5.0", default-features = false, features = ["reqwest", "rustls-tls"] }
openidconnect   = { version = "4",   default-features = false, features = ["reqwest", "rustls-tls"] }
async-trait     = { version = "0.1" }
secrecy         = { version = "0.10", features = ["serde"] }   # v0.8 #1
auth-core       = { path = "crates/auth-core" }
```

Notes:
- `default-features = false` removes `oauth2`'s default `native-tls` so the
  whole workspace stays on `rustls` (consistent with `reqwest` features
  already in `[workspace.dependencies]`).
- `openidconnect = "4"` is the latest stable line tracking `oauth2 = "5"`
  (v0.6's "0.4" string was a typo).
- `secrecy` (v0.8 #1) wraps every byte of secret material so it never lands
  in `Debug`, `tracing`, or panic output. `JwtIssuer.secret`, `ProviderCreds.client_secret`,
  and the flow-cookie HMAC key all use `SecretBox<Vec<u8>>` / `SecretString`.

### 1.2 `crates/auth-core/Cargo.toml`

```toml
[package]
name         = "auth-core"
version      = { workspace = true }
edition      = { workspace = true }
rust-version = { workspace = true }
authors      = { workspace = true }
license      = { workspace = true }
repository   = { workspace = true }

[dependencies]
oauth2        = { workspace = true }
openidconnect = { workspace = true }
async-trait   = { workspace = true }
jsonwebtoken  = { workspace = true }
serde         = { workspace = true }
serde_json    = { workspace = true }
thiserror     = { workspace = true }
figment       = { workspace = true }
ulid          = { workspace = true }
chrono        = { workspace = true }
tracing       = { workspace = true }
secrecy       = { workspace = true }                # v0.8 #1
url           = "2"
reqwest       = { workspace = true }

# Re-use the platform's TenantContext / PlanTier so we don't fork the type.
agent-core    = { path = "../agent-core" }
# Shared JwtSecret helper (v0.8 #2) — single source of truth for JWT_SECRET.
common        = { path = "../common" }

[dev-dependencies]
tokio    = { workspace = true }
wiremock = "0.6"
```

> Reusing `agent-core::{PlanTier, TenantContext, TenantClaims}` is a deliberate
> tightening of v0.6 — it guarantees that `auth-core` cannot drift from the
> middleware's claim shape.

### 1.3 Directory layout

```
crates/auth-core/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs
│   ├── error.rs
│   ├── profile.rs          # UserProfile (provider-normalised)
│   ├── tenant.rs           # TenantResolver trait + EmailDomainTenantResolver
│   ├── jwt.rs              # JwtIssuer (HS256, emits agent_core::TenantClaims)
│   ├── config.rs           # AuthConfig (figment, mirrors common::config pattern)
│   ├── service.rs          # AuthService — orchestrates provider + resolver + issuer (#[tracing::instrument])
│   ├── provider.rs         # OAuthProvider trait, FlowState, registry
│   └── providers/
│       ├── mod.rs
│       ├── google.rs       # openidconnect (full OIDC, JWKS-validated)
│       ├── github.rs       # plain oauth2 + GET /user + GET /user/emails
│       └── microsoft.rs    # openidconnect (multi-tenant common endpoint)
└── tests/
    ├── google_flow.rs       # wiremock OIDC happy/error paths
    ├── github_flow.rs
    └── tenant_resolver.rs
```

### 1.4 Type-level contracts (exact signatures)

`error.rs` (v0.8 #4 — `#[non_exhaustive]`, real `source` chains):
```rust
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AuthError {
    #[error("oauth request failed")]
    OAuthRequest(#[source] Box<dyn std::error::Error + Send + Sync>),
    #[error("oauth token exchange: {0}")]
    OAuthExchange(String),
    #[error("oidc discovery / verification")]
    Oidc(#[source] Box<dyn std::error::Error + Send + Sync>),
    #[error("jwt")]
    Jwt(#[from] jsonwebtoken::errors::Error),
    #[error("http")]
    Http(#[from] reqwest::Error),
    #[error("tenant resolution: {0}")] Tenant(String),
    #[error("config: {0}")]            Config(String),
    #[error("unknown provider: {0}")]  UnknownProvider(String),
    #[error("invalid state")]          InvalidState,
    #[error("invalid pkce verifier")]  InvalidPkce,
}
pub type Result<T> = std::result::Result<T, AuthError>;

// Per-provider From impls funnel `oauth2::RequestTokenError<…>` etc. into
// `OAuthRequest` so call sites stay `?`-friendly while preserving the chain.
```

`profile.rs`:
```rust
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct UserProfile {
    pub provider: String,        // "google" | "github" | "microsoft"
    pub provider_user_id: String,
    pub email: String,
    pub email_verified: bool,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
}
```

`tenant.rs`:
```rust
use agent_core::{PlanTier, TenantContext};

#[async_trait::async_trait]
pub trait TenantResolver: Send + Sync {
    async fn resolve_or_create(
        &self,
        profile: &UserProfile,
        hint: Option<&str>,
    ) -> Result<ResolvedTenant>;
}

pub struct ResolvedTenant {
    pub tenant_id: String,       // free-form; ULID or slug
    pub plan: PlanTier,
}

/// Dev / single-process default. Maps `email` domain → tenant slug.
/// `personal-domains` (gmail.com, …) collapse to `personal`.
pub struct EmailDomainTenantResolver { /* config */ }
```

`jwt.rs` — emits the **existing** `agent_core::TenantClaims`, secret wrapped
with `secrecy` (v0.8 #1) and pulled via the shared helper (v0.8 #2):
```rust
use agent_core::TenantClaims;
use common::jwt::JwtSecret;             // v0.8 #2 (new tiny module)
use secrecy::{ExposeSecret, SecretBox}; // v0.8 #1

pub struct JwtIssuer {
    secret: SecretBox<Vec<u8>>,
    ttl: chrono::Duration,
}

impl JwtIssuer {
    /// Constructs from the shared `JwtSecret` so the issuer and `mw/tenant.rs`
    /// can never disagree about which env var is authoritative.
    pub fn new(secret: JwtSecret, ttl: chrono::Duration) -> Self;
    pub fn from_env() -> Result<Self>;   // delegates to JwtSecret::from_env
    pub fn issue(&self, profile: &UserProfile, tenant: &ResolvedTenant) -> Result<String>;
    // sub = profile.email (matches existing middleware contract)
}
```

**v0.8 #2 — new shared module** `crates/common/src/jwt.rs`:
```rust
use secrecy::{ExposeSecret, SecretBox};

#[derive(Clone)]
pub struct JwtSecret(SecretBox<Vec<u8>>);

impl JwtSecret {
    pub fn from_env() -> Option<Self> {
        std::env::var("JWT_SECRET").ok()
            .filter(|s| !s.is_empty())
            .map(|s| Self(SecretBox::new(Box::new(s.into_bytes()))))
    }
    pub fn expose(&self) -> &[u8] { self.0.expose_secret() }
}
```
[mw/tenant.rs](crates/agent-gateway/src/mw/tenant.rs#L20-L46) is migrated in
Phase B to call `JwtSecret::from_env()` instead of reading `JWT_SECRET`
directly — semantics identical, drift impossible.

`provider.rs`:
```rust
pub struct FlowStart {
    pub authorization_url: url::Url,
    pub state: String,            // CSRF state, opaque
    pub pkce_verifier: String,    // caller stores (cookie/session) & echoes back
    pub nonce: Option<String>,    // OIDC only
}

#[async_trait::async_trait]
pub trait OAuthProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn start(&self) -> Result<FlowStart>;
    async fn finish(
        &self,
        code: &str,
        pkce_verifier: &str,
        nonce: Option<&str>,
    ) -> Result<UserProfile>;
}
```

`service.rs` — provider registry stays a plain `HashMap` (v0.8 #5: no premature
factory abstraction); both entry points are `#[tracing::instrument]` (v0.8 #3):
```rust
pub struct AuthService {
    providers: std::collections::HashMap<String, Box<dyn OAuthProvider>>,
    resolver:  Box<dyn TenantResolver>,
    issuer:    JwtIssuer,
    metrics:   AuthMetrics,            // login_success_total, login_failure_total{reason}
}

pub struct LoginToken {
    pub jwt: String,
    pub profile: UserProfile,
    pub tenant: ResolvedTenant,
}

impl AuthService {
    #[tracing::instrument(skip(self), fields(provider))]
    pub fn start_login(&self, provider: &str) -> Result<FlowStart>;

    #[tracing::instrument(skip(self, code, pkce_verifier, nonce), fields(provider, tenant_id))]
    pub async fn finish_login(
        &self,
        provider: &str,
        code: &str,
        pkce_verifier: &str,
        nonce: Option<&str>,
        tenant_hint: Option<&str>,
    ) -> Result<LoginToken>;
}
```

`AuthMetrics` is constructed from the existing `prometheus::Registry` already
threaded through `agent-gateway::main` ([main.rs](crates/agent-gateway/src/main.rs#L36-L41)):
```rust
pub struct AuthMetrics {
    pub login_success: prometheus::IntCounterVec,  // labels: provider
    pub login_failure: prometheus::IntCounterVec,  // labels: provider, reason
    pub flow_started:  prometheus::IntCounterVec,  // labels: provider
}
impl AuthMetrics {
    pub fn register(registry: &prometheus::Registry) -> Self;
}
```

`config.rs` — `figment` pattern identical to
[common/src/config/mod.rs](crates/common/src/config/mod.rs#L53-L61):

```rust
use secrecy::SecretString;       // v0.8 #1

#[derive(Debug, Clone, serde::Deserialize)]
pub struct AuthConfig {
    /// Optional override; otherwise `common::jwt::JwtSecret::from_env()` wins (v0.8 #2).
    pub jwt_secret: Option<SecretString>,
    pub jwt_ttl_secs: u64,                     // default 900 (15 min)
    pub base_url: String,                      // for redirect_uri construction
    pub providers: ProvidersConfig,
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
pub struct ProvidersConfig {
    pub google:    Option<ProviderCreds>,
    pub github:    Option<ProviderCreds>,
    pub microsoft: Option<ProviderCreds>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ProviderCreds {
    pub client_id: String,
    pub client_secret: SecretString,           // v0.8 #1 — never logged
}

impl AuthConfig {
    pub fn load() -> Result<Self> {
        figment::Figment::new()
            .merge(figment::providers::Toml::file("config.toml"))
            // v0.8 #7 — accept legacy CONUSAI_ prefix first (so `JWT_SECRET`,
            // `CONUSAI_BASE_URL`, etc. work during the transition window),
            // then let the explicit auth-scoped prefix override.
            .merge(figment::providers::Env::prefixed("CONUSAI_").split("__"))
            .merge(figment::providers::Env::prefixed("CONUSAI_AUTH__").split("__"))
            .extract()
            .map_err(|e| AuthError::Config(e.to_string()))
    }
}
```

Env-var examples (documented in `crates/auth-core/README.md`):
```
CONUSAI_AUTH__JWT_SECRET=...
CONUSAI_AUTH__JWT_TTL_SECS=900
CONUSAI_AUTH__BASE_URL=https://app.example.com
CONUSAI_AUTH__PROVIDERS__GOOGLE__CLIENT_ID=...
CONUSAI_AUTH__PROVIDERS__GOOGLE__CLIENT_SECRET=...
```

### 1.5 Implementation order (1 file = 1 commit, in order)

1. `error.rs` → `profile.rs` → `tenant.rs` (compiles standalone)
2. `jwt.rs` (with unit tests: round-trip → `agent_core::TenantClaims`)
3. `config.rs` (figment unit test with `Jail::expect_with`)
4. `provider.rs` (trait only — no providers yet)
5. `providers/google.rs` first (most complete OIDC reference)
6. `providers/microsoft.rs` (copy of google with discovery URL change)
7. `providers/github.rs` (oauth2 + manual profile fetch via `reqwest`)
8. `service.rs` (composes all above)
9. `lib.rs` re-exports + `pub fn build_auth_service(cfg, resolver) -> AuthService`
10. `tests/*.rs` with `wiremock` doubling each IdP

### 1.6 Phase A acceptance

- `cargo build -p auth-core` clean.
- `cargo test -p auth-core --all-features` (≥ 12 tests; one happy path + one
  CSRF-fail + one expired-code per provider; resolver email-domain matrix).
- **v0.8 #8** — `tests/jwt_contract.rs`: build a `TenantContext` via the real
  decode path used by [mw/tenant.rs](crates/agent-gateway/src/mw/tenant.rs#L43-L52)
  from a token issued by `JwtIssuer`. Pinned regression: shape change in
  `TenantClaims` will fail this test before any HTTP test does.
- `cargo clippy -p auth-core --all-targets -- -D warnings`.
- `cargo doc -p auth-core --no-deps` warning-free.
- No `Debug`/`Display` impl on any type that holds an unwrapped secret
  (enforced by `secrecy` types — v0.8 #1).
- README documents env vars + 30-LOC "add a provider" recipe.

---

## 2. Phase B — Wire `AuthService` into `agent-gateway`

**Goal:** Add the OAuth start/callback HTTP routes; persist outcome into the
existing `conusai_session` cookie *and* return the JWT for API clients.
**No** changes to `mw/tenant.rs`, `protected_router`, or `TenantClaims`.

### 2.1 New module

```
crates/agent-gateway/src/auth/
├── mod.rs
├── routes.rs
├── handlers.rs
└── flow_state.rs   # short-lived signed cookie that holds (state, pkce, nonce, provider, return_to)
```

`flow_state.rs` reuses the existing HMAC pattern from
[ui/session.rs](crates/agent-gateway/src/ui/session.rs#L60-L80) (same
`UI_SESSION_KEY`, different cookie name `conusai_oauth_flow`, 10 min TTL).

### 2.2 Routes (mounted on `public_router`)

[routes/mod.rs](crates/agent-gateway/src/routes/mod.rs#L21-L26) gains:

```rust
pub fn public_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/health", get(health::health))
        .route("/v1/files/{token}", get(files::download))
        .merge(crate::auth::routes::auth_router())   // ← NEW
}
```

`auth/routes.rs`:
```rust
pub fn auth_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/v1/auth/providers",            get(handlers::list_providers))
        .route("/v1/auth/{provider}/login",     get(handlers::start_login))
        .route("/v1/auth/{provider}/callback",  get(handlers::finish_login))
        .route("/v1/auth/logout",               post(handlers::logout))
        .route("/v1/auth/me",                   get(handlers::me))
        .layer(/* per-IP rate limiter from AppState.rate_limiter */)
}
```

Behaviour:
- `start_login` → 302 to provider authorization URL, sets `conusai_oauth_flow`
  HMAC cookie carrying `(provider, state, pkce_verifier, nonce, return_to)`,
  10 min, `HttpOnly; Secure; SameSite=Lax`.
- `finish_login` → validates flow cookie, calls `AuthService::finish_login`,
  on success:
    - Sets `conusai_session` cookie (existing HMAC, extended `SessionUser`).
    - If request `Accept: application/json` → returns
      `{ "token": "...", "expires_in": 900, "profile": {…} }` (for SPA/CLI).
    - Else → 302 to `return_to` (default `/`).
- `me` → echoes the resolved tenant + profile from the session cookie (used
  by frontend `+layout.server.ts` already).
- `logout` → clears both cookies, 204.

### 2.3 `SessionUser` extension (additive)

[ui/session.rs](crates/agent-gateway/src/ui/session.rs#L26-L55): add fields,
keeping `Deserialize` defaults so old cookies still parse.

```rust
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SessionUser {
    pub name: String,
    pub plan: String,
    pub exp: i64,
    #[serde(default)] pub email: Option<String>,
    #[serde(default)] pub provider: Option<String>,
    #[serde(default)] pub tenant_id: Option<String>,   // when set, overrides CONUSAI_UI_TENANT_ID
    #[serde(default)] pub avatar_url: Option<String>,
}

impl SessionUser {
    pub fn tenant_context(&self) -> TenantContext {
        let workspace_root = std::env::var("CONUSAI_WORKSPACE_ROOT")
            .unwrap_or_else(|_| "/tmp/conusai/workspaces".into());
        let tenant_id = self.tenant_id.clone()
            .or_else(|| std::env::var("CONUSAI_UI_TENANT_ID").ok())
            .unwrap_or_else(|| "dev".into());
        TenantContext::new(tenant_id, Some(self.email.clone().unwrap_or_else(|| self.name.clone())),
                           self.plan_tier(), workspace_root)
    }
}
```

This makes [mw/tenant.rs](crates/agent-gateway/src/mw/tenant.rs#L60-L88) dev-mode
path *automatically* honour the OAuth-issued tenant — no middleware change.

### 2.4 `AppState` extension

[state.rs](crates/agent-gateway/src/state.rs#L14-L34): add

```rust
/// None when no provider is configured (dev mode falls back to fake login).
pub auth_service: Option<Arc<auth_core::AuthService>>,
/// Shared with the metrics handler in main.rs (v0.8 #3).
pub prom_registry: Arc<prometheus::Registry>,
```

`AppState::from_env` (now `from_env(prom_registry: Arc<Registry>)`) calls
`auth_core::AuthConfig::load()`; on `AuthError::Config` (missing `[providers]`),
logs a warning and stores `None`. The Prometheus registry is threaded in so
`AuthMetrics::register` can hang counters off the same exposition surface as
[main.rs](crates/agent-gateway/src/main.rs#L36-L41). This preserves the current
"no env vars needed for `cargo run`" UX.

[mw/tenant.rs](crates/agent-gateway/src/mw/tenant.rs#L20) is migrated to
`common::jwt::JwtSecret::from_env()` (v0.8 #2) — pure refactor, identical
behaviour, eliminates the duplicated env read.

### 2.5 `main.rs` mount

[main.rs](crates/agent-gateway/src/main.rs#L43-L62) — `public_router()` is
already merged; nothing to change beyond the `routes/mod.rs` edit.

### 2.6 Phase B acceptance

- `cargo test -p agent-gateway` green (existing tests untouched).
- New integration test (`tests/auth_routes.rs`) using `wiremock` to fake an
  IdP discovery + token + userinfo, full happy path:
  `GET /v1/auth/google/login` → 302 → call back to
  `GET /v1/auth/google/callback?code=…&state=…` with the flow cookie →
  receives `conusai_session` cookie → `GET /v1/audit` (protected) succeeds
  with that cookie via dev-mode middleware path.
- Old `Bearer <jwt>` flow still works after the `JwtSecret` refactor (v0.8 #2):
  same `JWT_SECRET`, same HS256 claim shape — verified by reusing the v0.8 #8
  contract test from `auth-core` against the running server.
- `/metrics` exposes `conusai_auth_login_success_total` and
  `conusai_auth_login_failure_total{reason}` (v0.8 #3) after a happy path.
- No regression in [ui/session.rs](crates/agent-gateway/src/ui/session.rs)
  cookie verification for old-shape cookies.

---

## 3. Phase C — Frontend (`apps/web`) wiring + docs

### 3.1 Provider buttons on the login page

[apps/web/src/routes/login/+page.server.ts](apps/web/src/routes/login/+page.server.ts) —
`load` adds:
```ts
const providers = await fetch(`${env.GATEWAY_URL}/v1/auth/providers`)
    .then(r => r.json()).catch(() => ({ providers: [] }));
return { greeting: timeGreeting(), providers: providers.providers };
```

[apps/web/src/routes/login/+page.svelte](apps/web/src/routes/login/+page.svelte) —
above the existing `<form method="POST">`, render one `<a>` per provider:
```svelte
{#each data.providers as p}
    <a class="oauth-btn" href="{env.PUBLIC_GATEWAY_URL}/v1/auth/{p.id}/login?return_to=/">
        <img src="/icons/{p.id}.svg" alt=""> Continue with {p.label}
    </a>
{/each}
{#if data.providers.length === 0}
    <!-- existing dev fake-login form, unchanged -->
{/if}
```

The dev form keeps working when no providers are configured — zero-config
local dev is preserved.

### 3.2 SvelteKit ↔ gateway session bridging

`apps/web/src/lib/server/session.ts` already shares the HMAC key with
[ui/session.rs](crates/agent-gateway/src/ui/session.rs#L57-L63). After Phase B
the gateway sets `conusai_session` directly on the OAuth callback response, so
SvelteKit's `verify()` will read the same cookie automatically. **No changes
to the existing `sign`/`verify` helpers** — only update `SessionUser` TS type
to mirror the new optional fields:

```ts
export interface SessionUser {
    name: string;
    plan: string;
    exp: number;
    email?: string;
    provider?: string;
    tenant_id?: string;
    avatar_url?: string;
}
```

### 3.3 `GatewayClient` (v0.8 #6 — was `BackendJwtAdapter` in v0.7)

Create `apps/web/src/lib/server/backend.ts`:

```ts
export class GatewayClient {
    constructor(private baseUrl: string) {}

    /** Generic fetch against the gateway. Pass `jwt` for `/v1/*` (protected),
     *  omit it for `/ui/*` (cookie-authenticated). */
    async apiFetch(path: string, init: RequestInit = {}, jwt?: string) {
        const headers = new Headers(init.headers);
        if (jwt) headers.set('Authorization', `Bearer ${jwt}`);
        return fetch(`${this.baseUrl}${path}`, { ...init, headers });
    }
}

export const gateway = new GatewayClient(env.GATEWAY_URL);
```

The broader `GatewayClient` / `apiFetch` naming is reusable for future SSR
loaders against `/v1/*`. SSR can call `POST /v1/auth/exchange` (Phase D — not
required for v0.8) to get a JWT from a session cookie. For now, SSR proxies
to `/ui/*` which is already session-cookie authenticated.

### 3.4 Logout

[apps/web/src/routes/logout/+page.server.ts](apps/web/src/routes/logout/+page.server.ts) —
add a server-side `POST` that calls `/v1/auth/logout` on the gateway in
addition to clearing the SvelteKit cookie.

### 3.5 Docs

- `docs/backend/auth.md` (NEW) — ADR style:
    - architecture (mermaid: browser → gateway `/v1/auth/*` → IdP → callback → cookie/JWT → middleware → protected route)
    - config table (env vars)
    - "add a provider" 30-LOC walkthrough
    - ZITADEL migration sketch (drop-in `OAuthProvider` + `TenantResolver`)
- `docs/web/arch.md` — add **Authentication** section linking to the above.
- `README.md` (root) — link `docs/backend/auth.md` from the security section.

### 3.6 CI

`.github/workflows/*` (whichever runs `cargo test`):
- add `cargo test -p auth-core` to the matrix.
- add `cargo deny check advisories` (already required by Phase A acceptance).

### 3.7 Phase C acceptance

- `pnpm --filter web run check` clean.
- Manual: with no env vars, `pnpm dev` shows the existing fake-login form
  (no regression); with `CONUSAI_AUTH__PROVIDERS__GOOGLE__*` set, the
  Google button appears and end-to-end login lands the user on `/`.
- axe-core / Lighthouse scores unchanged on `/login`.
- `docs/backend/auth.md` reviewed; mermaid renders.

---

## 4. Phase D — Optional follow-ups (post-v0.7, deferred)

Captured here so they are not lost; **not** part of the merge gate.

1. **JWKS / RS256 mode** — second `JwtIssuer` impl using a key pair, plus
   `mw/tenant.rs` extension to support `Algorithm::RS256` via JWKS URL.
2. **`/v1/auth/exchange` (cookie → JWT)** — enables SvelteKit SSR to call
   `/v1/*` directly without the `/ui/*` proxy.
3. **Refresh tokens** — store hashed refresh handle in Qdrant
   (`auth_refresh_<tenant_id>`); rotate on use.
4. **ZITADEL `OAuthProvider`** — wraps the `zitadel` crate; keeps the
   `OAuthProvider` trait; replaces `EmailDomainTenantResolver` with a
   `ZitadelTenantResolver` reading orgs.
5. **DB-backed `TenantResolver`** — sqlx + Postgres + RLS hook (currently
   blocked by no Postgres in the stack — see [docker-compose.yml](docker-compose.yml)).

---

## 5. Sequencing, PRs, and tracking

```
PR-A  crates/auth-core + crates/common::jwt (Phase A)   — self-contained
      includes v0.8 #1 (secrecy), #2 (JwtSecret), #4 (errors), #7 (config), #8 (contract test)
PR-B  agent-gateway/auth/* + mw/tenant.rs migration to JwtSecret (Phase B)
      includes v0.8 #2 wiring, #3 (tracing + metrics)
PR-C  apps/web login + docs (Phase C)
      includes v0.8 #6 (GatewayClient rename)
```

Each PR:
- references this document's section number,
- ships its own acceptance gate from §1.6 / §2.6 / §3.7,
- updates [docs/web/tasks/frontend-improve.md](docs/web/tasks/frontend-improve.md)
  Phase 3 status when PR-C merges.

## 6. Risk register (final)

| Risk | Mitigation |
|---|---|
| `JWT_SECRET` mismatch between issuer and middleware | **v0.8 #2** — both call `common::jwt::JwtSecret::from_env()`; refactor of [mw/tenant.rs](crates/agent-gateway/src/mw/tenant.rs#L20) lands in PR-B |
| Secret leakage via `Debug` / panic / tracing logs | **v0.8 #1** — every secret wrapped in `secrecy::SecretBox` / `SecretString`; no plain `String` for secrets anywhere |
| Tenant leakage via guessable tenant_id | `EmailDomainTenantResolver` slugs via `slug::slugify` + collapses personal domains; `mw/tenant.rs` is the single trust boundary |
| OAuth state / PKCE replay | Flow cookie is HMAC-signed, 10 min TTL, cleared on first use |
| Cookie SameSite breakage on cross-site SSR | All cookies `SameSite=Lax`; gateway and web served from same eTLD+1 in prod |
| Provider drift (e.g. GitHub primary email) | `providers/github.rs` integration test pins the response shape via `wiremock` |
| `TenantClaims` shape regression | **v0.8 #8** — `tests/jwt_contract.rs` round-trips issuer → middleware decode |
| Future ZITADEL migration | Trait-based design — drop-in `OAuthProvider` + `TenantResolver` impls, no callers change |

---

**Definition of done (whole epic):**
1. All three PRs merged; CI green including `cargo test -p auth-core --all-features`
   (includes the v0.8 #8 contract test) and the gateway `auth_routes`
   integration test.
2. `docs/backend/auth.md` published and linked from root README; documents
   the shared `common::jwt::JwtSecret` helper and the tracing + Prometheus
   surface (v0.8 #2, #3).
3. Manual end-to-end login on staging with at least one real IdP returns a
   `conusai_session` cookie whose embedded `tenant_id` is honoured by
   [mw/tenant.rs](crates/agent-gateway/src/mw/tenant.rs) on a subsequent
   `/v1/audit` request, and `/metrics` increments
   `conusai_auth_login_success_total{provider="…"}`.
4. Dev `pnpm dev` with no env vars still works (fake-login fallback).
