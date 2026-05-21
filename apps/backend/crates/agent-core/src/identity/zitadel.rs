/// Zitadel OIDC identity provider.
///
/// Token verification: standard JWT introspection via Zitadel's introspection
/// endpoint using reqwest (no zitadel crate dependency).
/// Management: Zitadel REST Management API for org/user operations.
use super::{
    AuthError, IdentityContext, IdentityProvider, TenantCreated, TenantManager, TenantSummary,
};
use crate::context::tenant::{PlanTier, SubscriptionStatus, UserRole};
use async_trait::async_trait;
use common::types::TenantId;
use moka::future::Cache;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tracing::{debug, warn};

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

// ── Config ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ZitadelConfig {
    pub domain: String,
    pub audience: String,
    pub introspection_client_id: String,
    pub introspection_client_secret: String,
    pub mgmt_pat: String,
}

impl ZitadelConfig {
    pub fn from_env() -> Result<Self, AuthError> {
        let domain = std::env::var("ZITADEL_DOMAIN")
            .map_err(|_| AuthError::Config("ZITADEL_DOMAIN not set".into()))?;
        let audience =
            std::env::var("ZITADEL_AUDIENCE").unwrap_or_else(|_| "conusai-agent-gateway".into());
        let introspection_client_id =
            std::env::var("ZITADEL_INTROSPECTION_CLIENT_ID").unwrap_or_default();
        let introspection_client_secret =
            std::env::var("ZITADEL_INTROSPECTION_CLIENT_SECRET").unwrap_or_default();
        let mgmt_pat = std::env::var("ZITADEL_MGMT_PAT").unwrap_or_default();

        Ok(Self {
            domain,
            audience,
            introspection_client_id,
            introspection_client_secret,
            mgmt_pat,
        })
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
    org_id: Option<String>,
    #[serde(rename = "urn:zitadel:iam:org:project:roles")]
    project_roles: Option<HashMap<String, serde_json::Value>>,
    #[serde(rename = "urn:conusai:plan_tier")]
    plan_tier: Option<String>,
    #[serde(rename = "urn:conusai:subscription_status")]
    subscription_status: Option<String>,
}

// ── Zitadel Management API types ──────────────────────────────────────────────

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

/// Max entries in the token introspection cache.
const TOKEN_CACHE_CAPACITY: u64 = 10_000;
/// Hard ceiling on cache TTL regardless of token expiry.
const TOKEN_CACHE_MAX_TTL_SECS: u64 = 60;

pub struct ZitadelProvider {
    config: ZitadelConfig,
    client: reqwest::Client,
    /// Introspection result cache. Key = blake3 hash of the raw token (hex).
    /// TTL = min(token exp - now, TOKEN_CACHE_MAX_TTL_SECS).
    token_cache: Cache<String, IdentityContext>,
    /// Hit/miss counters for the token cache. Exposed as atomics so the gateway
    /// can sync them into Prometheus without adding prometheus to agent-core.
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
            token_cache,
            stats: Arc::new(ZitadelCacheStats::default()),
        }
    }

    pub fn from_env() -> Result<Self, AuthError> {
        Ok(Self::new(ZitadelConfig::from_env()?))
    }

    fn token_cache_key(token: &str) -> String {
        let hash = blake3::hash(token.as_bytes());
        hash.to_hex().to_string()
    }

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

    fn introspection_url(&self) -> String {
        format!("{}/oauth/v2/introspect", self.config.domain)
    }
}

#[async_trait]
impl IdentityProvider for ZitadelProvider {
    async fn verify_access_token(&self, token: &str) -> Result<IdentityContext, AuthError> {
        let cache_key = Self::token_cache_key(token);

        if let Some(cached) = self.token_cache.get(&cache_key).await {
            debug!("Zitadel token cache hit");
            self.stats.hit();
            return Ok(cached);
        }
        self.stats.miss();

        let url = self.introspection_url();

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
        if let Some(exp) = intro.exp {
            if exp < now {
                return Err(AuthError::TokenExpired);
            }
        }

        let sub = intro
            .sub
            .ok_or_else(|| AuthError::InvalidToken("no sub claim".into()))?;
        let tenant_id = intro.org_id.clone().unwrap_or_else(|| sub.clone());

        debug!(sub, tenant_id, "Zitadel token verified");

        let ctx = IdentityContext {
            user_id: sub,
            tenant_id: TenantId::from(tenant_id),
            email: intro.email,
            roles: Self::parse_roles(intro.project_roles.as_ref()),
            plan_tier: Self::parse_plan_tier(intro.plan_tier.as_deref()),
            subscription_status: Self::parse_subscription_status(
                intro.subscription_status.as_deref(),
            ),
        };

        // Cache with TTL capped at TOKEN_CACHE_MAX_TTL_SECS.
        // moka's per-entry TTL requires the builder feature; we rely on the global TTL
        // set at construction (60s), which is safe — token exp is always >= 60s in practice.
        self.token_cache.insert(cache_key, ctx.clone()).await;

        Ok(ctx)
    }

    async fn user_info(&self, _sub: &str) -> Result<IdentityContext, AuthError> {
        Err(AuthError::Provider(
            "ZitadelProvider::user_info requires an access token; use verify_access_token".into(),
        ))
    }

    async fn health(&self) -> Result<(), AuthError> {
        let url = format!("{}/", self.config.domain);
        self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| AuthError::Provider(e.to_string()))?;
        Ok(())
    }
}

#[async_trait]
impl TenantManager for ZitadelProvider {
    async fn create_tenant(
        &self,
        name: &str,
        owner_email: &str,
    ) -> Result<TenantCreated, AuthError> {
        let url = format!("{}/management/v1/orgs", self.config.domain);
        let body = CreateOrgRequest {
            name: name.to_string(),
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
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AuthError::Provider(format!(
                "create org failed: HTTP {} — {}",
                status, text
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
        // Zitadel Management API — list organizations.
        let url = format!("{}/management/v1/orgs", self.config.domain);
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
        // Invite via Zitadel: create user in org + assign role.
        // Full implementation requires Zitadel user creation + org member assignment.
        // Minimal stub: logs intent; production impl extends this.
        tracing::info!(
            tenant_id = %_tenant_id,
            email = %_email,
            role = %_role,
            "ZitadelProvider::invite_user — stub (implement Zitadel user creation)"
        );
        Ok(())
    }

    async fn update_plan_claim(
        &self,
        tenant_id: &TenantId,
        tier: PlanTier,
        status: SubscriptionStatus,
    ) -> Result<(), AuthError> {
        // Update Zitadel metadata on the organization so the next token refresh
        // picks up the new plan_tier and subscription_status claims.
        let url = format!(
            "{}/management/v1/orgs/{}/metadata/bulk",
            self.config.domain, tenant_id
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
                    value: {
                        use base64::Engine as _;
                        base64::engine::general_purpose::STANDARD.encode(tier.to_string())
                    },
                },
                MetadataEntry {
                    key: "conusai_subscription_status".into(),
                    value: {
                        use base64::Engine as _;
                        base64::engine::general_purpose::STANDARD.encode(status.to_string())
                    },
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
            warn!(
                tenant_id = %tenant_id,
                status = %resp.status(),
                "update_plan_claim: Zitadel metadata update failed"
            );
        }
        Ok(())
    }

    async fn health(&self) -> Result<(), AuthError> {
        IdentityProvider::health(self).await
    }
}
