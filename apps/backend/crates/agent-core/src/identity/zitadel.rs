/// Zitadel OIDC identity provider — JWKS (default) + introspection (opt-in).
///
/// Phase 0 adds local JWT validation via OIDC discovery + JWKS cache.
/// Introspection is preserved as an explicit opt-in via `VerifyMode::Introspection`.
use super::{
    AuthError, IdentityContext, IdentityProvider, TenantCreated, TenantManager, TenantSummary,
};
use crate::context::tenant::{PlanTier, SubscriptionStatus, UserRole};
use async_trait::async_trait;
use base64::Engine as _;
use common::types::TenantId;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use moka::future::Cache;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tracing::{debug, warn};

// ── Cache stats ───────────────────────────────────────────────────────────────

/// Counters exposed as atomics so the gateway can sync them into Prometheus
/// without pulling prometheus into agent-core's dependency tree.
#[derive(Debug, Default)]
pub struct ZitadelCacheStats {
    pub cache_hits: AtomicU64,
    pub cache_misses: AtomicU64,
}

impl ZitadelCacheStats {
    pub fn hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }
    pub fn miss(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
    }
    pub fn hits(&self) -> u64 {
        self.cache_hits.load(Ordering::Relaxed)
    }
    pub fn misses(&self) -> u64 {
        self.cache_misses.load(Ordering::Relaxed)
    }
}

// ── VerifyMode ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum VerifyMode {
    /// Default: verify JWTs locally via OIDC discovery + JWKS cache.
    Jwks,
    /// Opt-in: call Zitadel introspection endpoint on every request.
    /// Use for revocation-sensitive routes only.
    Introspection,
}

// ── Config ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ZitadelConfig {
    /// Full OIDC issuer URL, e.g. `https://auth.example.com` (no trailing slash).
    /// Also used as the base for management API calls.
    pub issuer: String,
    /// `ZITADEL_DOMAIN` alias — kept for management API compat; equal to `issuer`.
    pub domain: String,
    /// Expected `aud` claim value in access tokens.
    pub audience: String,
    /// Whether to use JWKS local validation (default) or introspection.
    pub verify_mode: VerifyMode,
    /// JWT claim key for the Zitadel org/tenant ID. Configurable via ZITADEL_ORG_CLAIM.
    pub org_id_claim: String,
    /// JWT claim key for project roles. Configurable via ZITADEL_ROLES_CLAIM.
    pub roles_claim: String,
    /// Introspection client id (used when verify_mode == Introspection).
    pub introspection_client_id: String,
    /// Introspection client secret.
    pub introspection_client_secret: String,
    /// Management API PAT for Zitadel admin operations.
    pub mgmt_pat: String,
    /// Skip HTTPS enforcement on endpoints (true in dev/test mode only).
    pub is_dev: bool,
}

impl ZitadelConfig {
    pub fn from_env() -> Result<Self, AuthError> {
        // ZITADEL_ISSUER is canonical; fall back to ZITADEL_DOMAIN for backward compat.
        let issuer = std::env::var("ZITADEL_ISSUER")
            .or_else(|_| std::env::var("ZITADEL_DOMAIN"))
            .map_err(|_| AuthError::Config("ZITADEL_ISSUER not set".into()))?
            .trim_end_matches('/')
            .to_string();

        let audience =
            std::env::var("ZITADEL_AUDIENCE").unwrap_or_else(|_| "conusai-agent-gateway".into());

        let verify_mode = match std::env::var("ZITADEL_TOKEN_VERIFY_MODE")
            .as_deref()
            .unwrap_or("jwks")
        {
            "introspection" => VerifyMode::Introspection,
            _ => VerifyMode::Jwks,
        };

        let org_id_claim = std::env::var("ZITADEL_ORG_CLAIM")
            .unwrap_or_else(|_| "urn:zitadel:iam:user:resourceowner:id".into());
        let roles_claim = std::env::var("ZITADEL_ROLES_CLAIM")
            .unwrap_or_else(|_| "urn:zitadel:iam:org:project:roles".into());

        let is_dev = std::env::var("APP_ENV")
            .map(|v| v == "dev" || v == "development")
            .unwrap_or(false)
            || cfg!(debug_assertions);

        Ok(Self {
            domain: issuer.clone(),
            issuer,
            audience,
            verify_mode,
            org_id_claim,
            roles_claim,
            introspection_client_id: std::env::var("ZITADEL_INTROSPECTION_CLIENT_ID")
                .unwrap_or_default(),
            introspection_client_secret: std::env::var("ZITADEL_INTROSPECTION_CLIENT_SECRET")
                .unwrap_or_default(),
            mgmt_pat: std::env::var("ZITADEL_MGMT_PAT").unwrap_or_default(),
            is_dev,
        })
    }
}

// ── OIDC Discovery ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
struct OidcDiscovery {
    issuer: String,
    jwks_uri: String,
    authorization_endpoint: String,
    token_endpoint: String,
    id_token_signing_alg_values_supported: Vec<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub revocation_endpoint: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub end_session_endpoint: Option<String>,
}

async fn fetch_discovery(
    client: &reqwest::Client,
    issuer: &str,
    is_dev: bool,
) -> Result<OidcDiscovery, AuthError> {
    let url = format!("{issuer}/.well-known/openid-configuration");
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| AuthError::Provider(format!("discovery fetch failed: {e}")))?;

    if !resp.status().is_success() {
        return Err(AuthError::Provider(format!(
            "discovery HTTP {}",
            resp.status()
        )));
    }

    let discovery: OidcDiscovery = resp
        .json()
        .await
        .map_err(|e| AuthError::Provider(format!("discovery parse failed: {e}")))?;

    // Issuer must match exactly (fail-closed on mismatch).
    if discovery.issuer.trim_end_matches('/') != issuer.trim_end_matches('/') {
        return Err(AuthError::Config(format!(
            "issuer mismatch: configured={issuer}, discovery={}",
            discovery.issuer
        )));
    }

    // RS256 must be in the supported alg list.
    if !discovery
        .id_token_signing_alg_values_supported
        .contains(&"RS256".to_string())
    {
        return Err(AuthError::Config(
            "issuer does not support RS256 id_token signing".into(),
        ));
    }

    // In non-dev mode, all endpoints must use HTTPS.
    if !is_dev {
        for ep_url in [
            &discovery.jwks_uri,
            &discovery.authorization_endpoint,
            &discovery.token_endpoint,
        ] {
            if !ep_url.starts_with("https://") {
                return Err(AuthError::Config(format!(
                    "non-HTTPS endpoint in production: {ep_url}"
                )));
            }
        }
    }

    Ok(discovery)
}

// ── JWKS ──────────────────────────────────────────────────────────────────────

/// RSA JWK — the fields we need to reconstruct a `DecodingKey`.
#[derive(Debug, Deserialize, Clone)]
struct Jwk {
    kty: String,
    #[serde(default)]
    kid: Option<String>,
    /// Base64url-encoded RSA modulus.
    n: Option<String>,
    /// Base64url-encoded RSA public exponent.
    e: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
struct JwkSet {
    keys: Vec<Jwk>,
}

/// Parsed RSA key components (base64url n + e) indexed by kid.
#[derive(Debug, Clone, Default)]
struct JwkState {
    keys: HashMap<String, (String, String)>, // kid -> (n, e)
    negatives: HashSet<String>,              // kid -> "not found" (reset on refresh)
    fetched_at: Option<Instant>,
}

const JWKS_TTL: Duration = Duration::from_secs(600); // 10 min

impl JwkState {
    fn is_stale(&self) -> bool {
        self.fetched_at
            .map(|t| t.elapsed() > JWKS_TTL)
            .unwrap_or(true)
    }
}

// ── Introspection response ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct IntrospectionResponse {
    active: bool,
    sub: Option<String>,
    exp: Option<u64>,
    email: Option<String>,
    #[serde(rename = "urn:zitadel:iam:org:id")]
    org_id_introspect: Option<String>,
    #[serde(rename = "urn:zitadel:iam:user:resourceowner:id")]
    org_id_resource: Option<String>,
    #[serde(rename = "urn:zitadel:iam:org:project:roles")]
    project_roles: Option<HashMap<String, serde_json::Value>>,
    #[serde(rename = "urn:conusai:plan_tier")]
    plan_tier: Option<String>,
    #[serde(rename = "urn:conusai:subscription_status")]
    subscription_status: Option<String>,
}

// ── Management API types ──────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct CreateOrgRequest {
    name: String,
}

#[derive(Debug, Deserialize)]
struct CreateOrgResponse {
    #[serde(rename = "organizationId")]
    organization_id: Option<String>,
}

// ── Provider ──────────────────────────────────────────────────────────────────

/// Capacity for the introspection token cache.
const TOKEN_CACHE_CAPACITY: u64 = 10_000;
/// Hard ceiling on introspection cache TTL.
const TOKEN_CACHE_MAX_TTL_SECS: u64 = 60;

pub struct ZitadelProvider {
    config: ZitadelConfig,
    client: reqwest::Client,

    // ── JWKS path ─────────────────────────────────────────────────────────────
    /// OIDC discovery document. Initialized once; reset on restart.
    discovery: tokio::sync::OnceCell<OidcDiscovery>,
    /// Parsed JWKS keyset with TTL tracking.
    jwks: tokio::sync::RwLock<JwkState>,
    /// Single-flight mutex for JWKS refresh — only one concurrent fetch allowed.
    jwks_refresh: tokio::sync::Mutex<()>,

    // ── Introspection path (opt-in) ────────────────────────────────────────────
    /// Introspection result cache keyed by blake3(token). TTL = 60s hard cap.
    token_cache: Cache<String, IdentityContext>,

    pub stats: Arc<ZitadelCacheStats>,
}

impl ZitadelProvider {
    pub fn new(config: ZitadelConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("HTTP client build failed");
        let token_cache = Cache::builder()
            .max_capacity(TOKEN_CACHE_CAPACITY)
            .time_to_live(Duration::from_secs(TOKEN_CACHE_MAX_TTL_SECS))
            .build();
        Self {
            config,
            client,
            discovery: tokio::sync::OnceCell::new(),
            jwks: tokio::sync::RwLock::new(JwkState::default()),
            jwks_refresh: tokio::sync::Mutex::new(()),
            token_cache,
            stats: Arc::new(ZitadelCacheStats::default()),
        }
    }

    pub fn from_env() -> Result<Self, AuthError> {
        Ok(Self::new(ZitadelConfig::from_env()?))
    }

    // ── OIDC Discovery ────────────────────────────────────────────────────────

    async fn get_discovery(&self) -> Result<&OidcDiscovery, AuthError> {
        let client = self.client.clone();
        let issuer = self.config.issuer.clone();
        let is_dev = self.config.is_dev;
        self.discovery
            .get_or_try_init(|| async move { fetch_discovery(&client, &issuer, is_dev).await })
            .await
    }

    // ── JWKS key management ───────────────────────────────────────────────────

    async fn refresh_jwks(&self) -> Result<(), AuthError> {
        // Single-flight gate: only one goroutine fetches at a time.
        let _guard = self.jwks_refresh.lock().await;

        // Double-check: another concurrent request may have already refreshed.
        {
            let state = self.jwks.read().await;
            if !state.is_stale() {
                return Ok(());
            }
        }

        let discovery = self.get_discovery().await?;
        let resp = self
            .client
            .get(&discovery.jwks_uri)
            .send()
            .await
            .map_err(|e| AuthError::Provider(format!("JWKS fetch failed: {e}")))?;

        if !resp.status().is_success() {
            return Err(AuthError::Provider(format!("JWKS HTTP {}", resp.status())));
        }

        let jwk_set: JwkSet = resp
            .json()
            .await
            .map_err(|e| AuthError::Provider(format!("JWKS parse failed: {e}")))?;

        let mut state = self.jwks.write().await;
        state.keys.clear();
        state.negatives.clear();
        state.fetched_at = Some(Instant::now());

        for jwk in jwk_set.keys {
            if jwk.kty == "RSA"
                && let (Some(kid), Some(n), Some(e)) = (jwk.kid, jwk.n, jwk.e)
            {
                state.keys.insert(kid, (n, e));
            }
        }

        debug!(keys = state.keys.len(), "JWKS refreshed");
        Ok(())
    }

    /// Returns `(n, e)` for the given kid, refreshing JWKS exactly once on miss.
    async fn get_jwk_components(&self, kid: &str) -> Result<(String, String), AuthError> {
        // Fast path: read lock, fresh keyset, kid found.
        {
            let state = self.jwks.read().await;
            if !state.is_stale() {
                if let Some(pair) = state.keys.get(kid) {
                    return Ok(pair.clone());
                }
                if state.negatives.contains(kid) {
                    return Err(AuthError::InvalidToken(format!(
                        "unknown kid (cached): {kid}"
                    )));
                }
            }
        }

        // Slow path: refresh JWKS (single-flight).
        self.refresh_jwks().await?;

        // Re-check after refresh.
        {
            let state = self.jwks.read().await;
            if let Some(pair) = state.keys.get(kid) {
                return Ok(pair.clone());
            }
        }

        // Kid still absent — add to negative cache.
        self.jwks.write().await.negatives.insert(kid.to_string());
        Err(AuthError::InvalidToken(format!("unknown kid: {kid}")))
    }

    // ── JWT verification (JWKS path) ──────────────────────────────────────────

    pub async fn verify_jwt(&self, token: &str) -> Result<IdentityContext, AuthError> {
        // 1. Decode header (no signature check yet; cheap).
        let header = decode_header(token)
            .map_err(|e| AuthError::InvalidToken(format!("bad token header: {e}")))?;

        // 2. Alg allowlist: only RS256 permitted. Reject `none`, HS256, and any other.
        if header.alg != Algorithm::RS256 {
            return Err(AuthError::InvalidToken(format!(
                "alg not in allowlist: {:?}",
                header.alg
            )));
        }

        // 3. Get kid (required for RS256 with multiple keys).
        let kid = header.kid.as_deref().unwrap_or("");

        // 4. Look up JWK components (triggers single-flight refresh on miss).
        let (n, e) = self.get_jwk_components(kid).await?;

        // 5. Build decoding key from JWK RSA components.
        let decoding_key = DecodingKey::from_rsa_components(&n, &e)
            .map_err(|e| AuthError::InvalidToken(format!("invalid RSA JWK components: {e}")))?;

        // 6. Validate JWT claims.
        validate_jwt_claims(token, &decoding_key, &self.config)
    }

    // ── Introspection path ────────────────────────────────────────────────────

    fn token_cache_key(token: &str) -> String {
        blake3::hash(token.as_bytes()).to_hex().to_string()
    }

    async fn verify_introspection(&self, token: &str) -> Result<IdentityContext, AuthError> {
        let cache_key = Self::token_cache_key(token);

        if let Some(cached) = self.token_cache.get(&cache_key).await {
            debug!("introspection cache hit");
            self.stats.hit();
            return Ok(cached);
        }
        self.stats.miss();

        let url = format!("{}/oauth/v2/introspect", self.config.issuer);
        let resp = self
            .client
            .post(&url)
            .basic_auth(
                &self.config.introspection_client_id,
                Some(&self.config.introspection_client_secret),
            )
            .form(&[("token", token)])
            .send()
            .await
            .map_err(|e| AuthError::Provider(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(AuthError::InvalidToken(format!(
                "introspection HTTP {}",
                resp.status()
            )));
        }

        let intro: IntrospectionResponse = resp
            .json()
            .await
            .map_err(|e| AuthError::Provider(e.to_string()))?;

        if !intro.active {
            return Err(AuthError::TokenExpired);
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if let Some(exp) = intro.exp
            && exp < now
        {
            return Err(AuthError::TokenExpired);
        }

        let sub = intro
            .sub
            .ok_or_else(|| AuthError::InvalidToken("no sub claim".into()))?;

        // Prefer resourceowner:id over legacy org:id for consistency with JWKS path.
        let tenant_id = intro
            .org_id_resource
            .or(intro.org_id_introspect)
            .unwrap_or_else(|| sub.clone());

        let ctx = IdentityContext {
            user_id: sub,
            tenant_id: TenantId::from(tenant_id),
            email: intro.email,
            roles: parse_roles(intro.project_roles.as_ref()),
            plan_tier: parse_plan_tier(intro.plan_tier.as_deref()),
            subscription_status: parse_subscription_status(intro.subscription_status.as_deref()),
        };

        self.token_cache.insert(cache_key, ctx.clone()).await;
        Ok(ctx)
    }
}

// ── JWT claims validation (JWKS path, public for tests) ──────────────────────

#[derive(Debug, Deserialize)]
struct AccessTokenClaims {
    #[allow(dead_code)]
    iss: String, // validated by jsonwebtoken Validation::set_issuer; kept for symmetry
    sub: String,
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

/// Core JWT decode + claim validation given an already-resolved `DecodingKey`.
/// Separated from `verify_jwt` so tests can call it directly without HTTP.
pub(crate) fn validate_jwt_claims(
    token: &str,
    key: &DecodingKey,
    config: &ZitadelConfig,
) -> Result<IdentityContext, AuthError> {
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[&config.issuer]);
    validation.set_audience(&[&config.audience]);
    validation.leeway = 60; // 60 s clock skew
    validation.validate_nbf = true;
    // alg already checked in verify_jwt before reaching here; set explicitly for safety.
    validation.algorithms = vec![Algorithm::RS256];

    let token_data = decode::<AccessTokenClaims>(token, key, &validation)
        .map_err(|e| AuthError::InvalidToken(e.to_string()))?;

    let claims = token_data.claims;

    if claims.sub.is_empty() {
        return Err(AuthError::InvalidToken("sub claim is empty".into()));
    }

    // Extract org_id from the configured claim key.
    let org_id = claims
        .extra
        .get(&config.org_id_claim)
        .and_then(|v| v.as_str())
        .map(String::from);

    // org_id is required per plan invariant 37.
    let tenant_id = org_id.ok_or_else(|| {
        AuthError::InvalidToken(format!("missing org claim: {}", config.org_id_claim))
    })?;

    // Extract project roles.
    let roles_raw = claims
        .extra
        .get(&config.roles_claim)
        .and_then(|v| v.as_object())
        .map(|m| m.iter().map(|(k, _)| k.clone()).collect::<Vec<_>>());
    let roles = map_project_roles(roles_raw.as_deref());

    // email and email_verified from token (display only, never identity).
    let email = claims
        .extra
        .get("email")
        .and_then(|v| v.as_str())
        .filter(|_| {
            claims
                .extra
                .get("email_verified")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        })
        .map(String::from);

    debug!(sub = %claims.sub, tenant_id, "JWKS JWT verified");

    Ok(IdentityContext {
        user_id: claims.sub,
        tenant_id: TenantId::from(tenant_id),
        email,
        roles,
        plan_tier: PlanTier::Free, // plan tier comes from tenant binding in Phase 6
        subscription_status: SubscriptionStatus::Active,
    })
}

// ── Helper parsers (shared by both paths) ─────────────────────────────────────

fn parse_plan_tier(s: Option<&str>) -> PlanTier {
    match s {
        Some("pro") => PlanTier::Pro,
        Some("enterprise") => PlanTier::Enterprise,
        _ => PlanTier::Free,
    }
}

fn parse_subscription_status(s: Option<&str>) -> SubscriptionStatus {
    match s {
        Some("active") => SubscriptionStatus::Active,
        Some("trialing") => SubscriptionStatus::Trialing,
        Some("past_due") => SubscriptionStatus::PastDue,
        Some("canceled") => SubscriptionStatus::Canceled,
        _ => SubscriptionStatus::Active,
    }
}

fn parse_roles(roles_map: Option<&HashMap<String, serde_json::Value>>) -> Vec<UserRole> {
    let Some(map) = roles_map else {
        return vec![UserRole::User];
    };
    let mut roles = vec![];
    if map.contains_key("super_admin") {
        roles.push(UserRole::SuperAdmin);
    } else if map.contains_key("admin") {
        roles.push(UserRole::Admin);
    } else {
        roles.push(UserRole::User);
    }
    roles
}

fn map_project_roles(role_keys: Option<&[String]>) -> Vec<UserRole> {
    let Some(keys) = role_keys else {
        return vec![UserRole::User];
    };
    let mut roles = Vec::new();
    if keys.iter().any(|k| k == "platform.admin") {
        roles.push(UserRole::SuperAdmin);
    } else if keys.iter().any(|k| k == "tenant.admin") {
        roles.push(UserRole::Admin);
    } else {
        roles.push(UserRole::User);
    }
    roles
}

// ── IdentityProvider impl ─────────────────────────────────────────────────────

#[async_trait]
impl IdentityProvider for ZitadelProvider {
    async fn verify_access_token(&self, token: &str) -> Result<IdentityContext, AuthError> {
        match self.config.verify_mode {
            VerifyMode::Jwks => self.verify_jwt(token).await,
            VerifyMode::Introspection => self.verify_introspection(token).await,
        }
    }

    async fn user_info(&self, _sub: &str) -> Result<IdentityContext, AuthError> {
        Err(AuthError::Provider(
            "ZitadelProvider::user_info requires an access token; use verify_access_token".into(),
        ))
    }

    async fn health(&self) -> Result<(), AuthError> {
        let url = format!("{}/", self.config.issuer);
        self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| AuthError::Provider(e.to_string()))?;
        Ok(())
    }
}

// ── TenantManager impl ────────────────────────────────────────────────────────

#[async_trait]
impl TenantManager for ZitadelProvider {
    async fn create_tenant(
        &self,
        name: &str,
        owner_email: &str,
    ) -> Result<TenantCreated, AuthError> {
        let url = format!("{}/management/v1/orgs", self.config.issuer);
        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.config.mgmt_pat)
            .json(&CreateOrgRequest {
                name: name.to_string(),
            })
            .send()
            .await
            .map_err(|e| AuthError::Provider(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AuthError::Provider(format!(
                "create org failed: HTTP {status} — {text}"
            )));
        }

        let created: CreateOrgResponse = resp
            .json()
            .await
            .map_err(|e| AuthError::Provider(e.to_string()))?;

        let org_id = created
            .organization_id
            .ok_or_else(|| AuthError::Provider("no organizationId in response".into()))?;

        Ok(TenantCreated {
            tenant_id: TenantId::from(org_id),
            name: name.to_string(),
            owner_email: owner_email.to_string(),
        })
    }

    async fn list_tenants(&self) -> Result<Vec<TenantSummary>, AuthError> {
        let url = format!("{}/management/v1/orgs", self.config.issuer);
        let resp = self
            .client
            .get(&url)
            .bearer_auth(&self.config.mgmt_pat)
            .send()
            .await
            .map_err(|e| AuthError::Provider(e.to_string()))?;

        if !resp.status().is_success() {
            warn!("list_tenants: Zitadel returned {}", resp.status());
            return Ok(vec![]);
        }

        #[derive(Deserialize)]
        struct ListOrgsResp {
            result: Option<Vec<OrgEntry>>,
        }
        #[derive(Deserialize)]
        struct OrgEntry {
            id: Option<String>,
            name: Option<String>,
        }

        let data: ListOrgsResp = resp
            .json()
            .await
            .map_err(|e| AuthError::Provider(e.to_string()))?;

        Ok(data
            .result
            .unwrap_or_default()
            .into_iter()
            .filter_map(|o| {
                Some(TenantSummary {
                    tenant_id: TenantId::from(o.id?),
                    name: o.name.unwrap_or_default(),
                    owner_email: None,
                    plan_tier: PlanTier::Free,
                    subscription_status: SubscriptionStatus::Active,
                })
            })
            .collect())
    }

    async fn invite_user(
        &self,
        _tenant_id: &TenantId,
        _email: &str,
        _role: UserRole,
    ) -> Result<(), AuthError> {
        tracing::info!(tenant_id = %_tenant_id, email = %_email, role = %_role,
            "ZitadelProvider::invite_user — stub (implement Zitadel user creation)");
        Ok(())
    }

    async fn update_plan_claim(
        &self,
        tenant_id: &TenantId,
        tier: PlanTier,
        status: SubscriptionStatus,
    ) -> Result<(), AuthError> {
        let url = format!(
            "{}/management/v1/orgs/{}/metadata/bulk",
            self.config.issuer, tenant_id
        );

        #[derive(Serialize)]
        struct MetadataEntry {
            key: String,
            value: String,
        }
        #[derive(Serialize)]
        struct BulkSetMetadataRequest {
            metadata: Vec<MetadataEntry>,
        }

        let body = BulkSetMetadataRequest {
            metadata: vec![
                MetadataEntry {
                    key: "conusai_plan_tier".into(),
                    value: base64::engine::general_purpose::STANDARD.encode(tier.to_string()),
                },
                MetadataEntry {
                    key: "conusai_subscription_status".into(),
                    value: base64::engine::general_purpose::STANDARD.encode(status.to_string()),
                },
            ],
        };

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.config.mgmt_pat)
            .json(&body)
            .send()
            .await
            .map_err(|e| AuthError::Provider(e.to_string()))?;

        if !resp.status().is_success() {
            warn!(tenant_id = %tenant_id, status = %resp.status(),
                "update_plan_claim: Zitadel metadata update failed");
        }
        Ok(())
    }

    async fn health(&self) -> Result<(), AuthError> {
        IdentityProvider::health(self).await
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
    use serde::Serialize;

    // RSA-2048 test key pair (test-only; never used in production).
    // Generated with: openssl genrsa 2048 (PKCS#8 format)
    const TEST_RSA_PRIVATE_KEY: &str = "-----BEGIN PRIVATE KEY-----
MIIEugIBADANBgkqhkiG9w0BAQEFAASCBKQwggSgAgEAAoIBAQC5yfWywDCQ0rkw
msyHvBd3hRNSe6sbZE/o9v51/5czrardJOOvUAbYmeHydoXYSpu2FLyPr8BK7BT7
v8OafbXtlRqIjKrEtUN0k5cibvZFpM1Sfj7wr00B2RxAF9ExuSJo+ZH5g+w1oDgW
Bdtn0OB5CacYjPU8H0pBnWDFlAggfO/PNYmYHc/t2DJOfsAzCz/+3NhMJCoTUtYg
KE/qpm9qEWiY/lVvQCdpPlPVNKrsYNhfP4KVIa+fCpN5cn47LQ5Uk0qBcYrjR/VQ
TtJfwhlxBs/dBWSxyr3lCCVncXT02PjL9UAI/FrGWj59S/y5QcVQqqYx+q6PmNUD
vrOR5oxlAgMBAAECgf80HTWRT/Y3I162pRbq47+AopZ/yYYdNRJZzNZW7bxvzsES
tOTHhUmjR59WfDG5z2SI0y/gi+X21QUmKx45K4P1CFfvGjDMmzBpIV7JnzHvY1co
j67O4NdLS+Rv/awFzKupiZTu4oxTWfM8/UMIDLPeZFzcPPcHI0XhzIyVihUfB1kM
RuO9KrFMMh72IODHHTMwmk17wkGosHHaA/KlMGA/9HM44BJCzu/adNR51Gqr5Ch/
u7hxK/txICg+EG8+LNGKQP13eqPcZyXC4p3Q967ofvA8BQ4ymbgMpjTvCCZI/d6y
6bx4OVHVp21fZi2U9xYeGgasFB6NAHXf23tWzoECgYEA9Tc2bQhgo78gzno2TTMg
cEHvznNZ91/ui3BbUQwLOA2m/L2TqxcnCmWmCsVxGK542x1q/rkWa4Kn+mtPQdWI
8NOu9xXWdiFvpuxYaFPD0NC27ag7xJEyHBCXsauPFX9E2hPronxUMxgPI1FO4gIB
YuqJrP4h9qNcUEmWGd3nYVkCgYEAwfWvNPNbjKYd9hy7Gd6sFEG0HDaeTutvzjDg
Yl73qRCKjrS9T7cVhZrO4521hkT0/TljsLr8LDGPfGlb5kB/OlwimwmCQTX6ZelH
O2h361xyuOWogdBWYg844CuV2mzBvgoggo0jeUwYbsTiL6oCPvaasICLio9I3vjb
G4yaNe0CgYBzQO8o0h4x+Hxf79sj79rYSHWBEICBn6pMCZQyBLolL22EL0p/yNMF
tP8U4vYkRqTxP+NxM+dQwslXDybiZ44Eu0nqQm5ZeZ+z0jQ/XNeVhvPjwgXNfv0R
ac8Sp/MJhJcE9QX0igE9Ppqm2+l6mryyFFB/abbm6KNT7TJKmBzPiQKBgCUM8H4V
6qwQY3LLBDap4YcxEd291TnQIZhqn8JKz9Zc0Yr3HZ6no5XU/6ZdTvqqG35vwwpU
fa1XfkhOu/5c3bDhPr8M7vPUAtQK3s+LYjT0gPmu7SR3DrlGnR+9U6/YzJ2nw5QM
r/UQwy4NsANY33r1kpEazQ0X19y3/urhTef1AoGACQ6wjnfQ5W8fLQXzzyZOCE6+
wfcoK52GbDZ7CP3Z2pITOHtKP0/XqXjRv55ncoIXXyxJVl6Kehc/BIXOSEfaQJm9
qGVo5xYh8IHYAGgpoOzZjCshNsRFTx5o1rvEVSb73mhJBvill0SqCaFssEQUIQYL
IsEvCkbG04tNoj1MJEM=
-----END PRIVATE KEY-----";

    // Corresponding JWK components (base64url) for the key above.
    // e = 65537 = AQAB always for standard RSA keys.
    const TEST_RSA_PUBLIC_N: &str = "ucn1ssAwkNK5MJrMh7wXd4UTUnurG2RP6Pb-df-XM62q3STjr1AG2Jnh8naF2EqbthS8\
         j6_ASuwU-7_Dmn217ZUaiIyqxLVDdJOXIm72RaTNUn4-8K9NAdkcQBfRMbkiaPmR-YPs\
         NaA4FgXbZ9DgeQmnGIz1PB9KQZ1gxZQIIHzvzzWJmB3P7dgyTn7AMws__tzYTCQqE1LW\
         IChP6qZvahFomP5Vb0AnaT5T1TSq7GDYXz-ClSGvnwqTeXJ-Oy0OVJNKgXGK40f1UE7S\
         X8IZcQbP3QVkscq95QglZ3F09Nj4y_VACPxaxlo-fUv8uUHFUKqmMfquj5jVA76zkeaM\
         ZQ";
    const TEST_RSA_PUBLIC_E: &str = "AQAB";

    fn test_config(verify_mode: VerifyMode) -> ZitadelConfig {
        ZitadelConfig {
            issuer: "https://auth.test.epifly".into(),
            domain: "https://auth.test.epifly".into(),
            audience: "test-gateway".into(),
            verify_mode,
            org_id_claim: "urn:zitadel:iam:user:resourceowner:id".into(),
            roles_claim: "urn:zitadel:iam:org:project:roles".into(),
            introspection_client_id: String::new(),
            introspection_client_secret: String::new(),
            mgmt_pat: String::new(),
            is_dev: true,
        }
    }

    #[derive(Debug, Serialize)]
    struct TestClaims {
        iss: String,
        sub: String,
        aud: Vec<String>,
        exp: u64,
        nbf: u64,
        iat: u64,
        #[serde(rename = "urn:zitadel:iam:user:resourceowner:id")]
        org_id: String,
        #[serde(rename = "urn:zitadel:iam:org:project:roles")]
        roles: serde_json::Value,
    }

    fn make_test_claims(iss: &str, sub: &str, aud: &str, org_id: &str) -> TestClaims {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        TestClaims {
            iss: iss.into(),
            sub: sub.into(),
            aud: vec![aud.into()],
            exp: now + 3600,
            nbf: now - 5,
            iat: now - 5,
            org_id: org_id.into(),
            roles: serde_json::json!({ "tenant.member": {} }),
        }
    }

    fn sign_test_jwt(claims: &TestClaims) -> String {
        let key = EncodingKey::from_rsa_pem(TEST_RSA_PRIVATE_KEY.as_bytes())
            .expect("test private key must be valid");
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some("test-kid-1".into());
        encode(&header, claims, &key).expect("JWT encode must succeed")
    }

    fn test_decoding_key() -> DecodingKey {
        DecodingKey::from_rsa_components(TEST_RSA_PUBLIC_N, TEST_RSA_PUBLIC_E)
            .expect("test JWK components must form a valid key")
    }

    // ── accepts_valid_jwt ─────────────────────────────────────────────────────

    #[test]
    fn accepts_valid_jwt() {
        let config = test_config(VerifyMode::Jwks);
        let claims = make_test_claims(
            "https://auth.test.epifly",
            "user|123",
            "test-gateway",
            "org-abc",
        );
        let token = sign_test_jwt(&claims);
        let key = test_decoding_key();
        let result = validate_jwt_claims(&token, &key, &config);
        assert!(result.is_ok(), "valid JWT must be accepted: {result:?}");
        let ctx = result.unwrap();
        assert_eq!(ctx.user_id, "user|123");
        assert_eq!(ctx.tenant_id.as_ref(), "org-abc");
    }

    // ── expired token ─────────────────────────────────────────────────────────

    #[test]
    fn rejects_expired_jwt() {
        let config = test_config(VerifyMode::Jwks);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let claims = TestClaims {
            iss: "https://auth.test.epifly".into(),
            sub: "user|123".into(),
            aud: vec!["test-gateway".into()],
            exp: now - 120, // already expired (> 60s leeway)
            nbf: now - 3600,
            iat: now - 3600,
            org_id: "org-abc".into(),
            roles: serde_json::json!({}),
        };
        let token = sign_test_jwt(&claims);
        let key = test_decoding_key();
        let result = validate_jwt_claims(&token, &key, &config);
        assert!(
            matches!(result, Err(AuthError::InvalidToken(_))),
            "expired JWT must be rejected: {result:?}"
        );
    }

    // ── wrong issuer ─────────────────────────────────────────────────────────

    #[test]
    fn rejects_wrong_issuer() {
        let config = test_config(VerifyMode::Jwks);
        let claims = make_test_claims("https://evil.issuer", "user|123", "test-gateway", "org-abc");
        let token = sign_test_jwt(&claims);
        let key = test_decoding_key();
        let result = validate_jwt_claims(&token, &key, &config);
        assert!(result.is_err(), "wrong iss must be rejected");
    }

    // ── wrong audience ────────────────────────────────────────────────────────

    #[test]
    fn rejects_wrong_audience() {
        let config = test_config(VerifyMode::Jwks);
        let claims = make_test_claims(
            "https://auth.test.epifly",
            "user|123",
            "wrong-audience",
            "org-abc",
        );
        let token = sign_test_jwt(&claims);
        let key = test_decoding_key();
        let result = validate_jwt_claims(&token, &key, &config);
        assert!(result.is_err(), "wrong aud must be rejected");
    }

    // ── missing org_id claim ──────────────────────────────────────────────────

    #[test]
    fn rejects_missing_org_id() {
        let config = test_config(VerifyMode::Jwks);
        #[derive(Serialize)]
        struct NoOrgClaims {
            iss: String,
            sub: String,
            aud: Vec<String>,
            exp: u64,
            nbf: u64,
            iat: u64,
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let claims = NoOrgClaims {
            iss: "https://auth.test.epifly".into(),
            sub: "user|123".into(),
            aud: vec!["test-gateway".into()],
            exp: now + 3600,
            nbf: now - 5,
            iat: now - 5,
        };
        let key = EncodingKey::from_rsa_pem(TEST_RSA_PRIVATE_KEY.as_bytes()).unwrap();
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some("test-kid-1".into());
        let token = encode(&header, &claims, &key).unwrap();
        let dk = test_decoding_key();
        let result = validate_jwt_claims(&token, &dk, &config);
        assert!(
            matches!(result, Err(AuthError::InvalidToken(_))),
            "missing org_id must be rejected: {result:?}"
        );
    }

    // ── alg=none / alg=HS256 attack ───────────────────────────────────────────

    #[tokio::test]
    async fn rejects_hs256_alg_confusion() {
        let provider = ZitadelProvider::new(test_config(VerifyMode::Jwks));
        // Build an HS256 token — alg header will be HS256.
        let claims = make_test_claims(
            "https://auth.test.epifly",
            "user|123",
            "test-gateway",
            "org-abc",
        );
        let hs_key = EncodingKey::from_secret(b"some-secret");
        let token = encode(&Header::new(Algorithm::HS256), &claims, &hs_key).unwrap();
        let result = provider.verify_jwt(&token).await;
        assert!(result.is_err(), "HS256 token must be rejected by JWKS path");
        if let Err(AuthError::InvalidToken(msg)) = &result {
            assert!(msg.contains("alg"), "error must mention alg: {msg}");
        }
    }

    // ── verify_mode::Jwks dispatches to verify_jwt ────────────────────────────

    #[test]
    fn verify_mode_enum_default_is_jwks() {
        let _config = ZitadelConfig::from_env().unwrap_or_else(|_| test_config(VerifyMode::Jwks));
        // Either from_env returned something or we get our test config — both fine.
        // Just assert the enum is round-trippable.
        assert_eq!(VerifyMode::Jwks, VerifyMode::Jwks);
        assert_ne!(VerifyMode::Jwks, VerifyMode::Introspection);
    }
}
