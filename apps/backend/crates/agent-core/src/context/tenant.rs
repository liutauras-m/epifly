use common::types::{TenantId, UserId};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Platform-level user role.  Carried in JWT claims and UI session cookies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UserRole {
    #[default]
    User,
    Admin,
    SuperAdmin,
}

impl std::fmt::Display for UserRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserRole::User => write!(f, "user"),
            UserRole::Admin => write!(f, "admin"),
            UserRole::SuperAdmin => write!(f, "super_admin"),
        }
    }
}

impl std::str::FromStr for UserRole {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "super_admin" => UserRole::SuperAdmin,
            "admin" => UserRole::Admin,
            _ => UserRole::User,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlanTier {
    Free,
    Pro,
    Enterprise,
}

impl PlanTier {
    pub fn max_tokens(&self) -> u64 {
        match self {
            PlanTier::Free => 4_096,
            PlanTier::Pro => 16_384,
            PlanTier::Enterprise => 128_000,
        }
    }

    /// Maximum agent tool-call rounds per request.
    pub fn max_turns(&self) -> u32 {
        match self {
            PlanTier::Free => 3,
            PlanTier::Pro => 8,
            PlanTier::Enterprise => 20,
        }
    }

    /// Requests per minute
    pub fn rate_limit_rpm(&self) -> u32 {
        match self {
            PlanTier::Free => 10,
            PlanTier::Pro => 60,
            PlanTier::Enterprise => 600,
        }
    }

    pub fn collection_prefix(&self) -> &'static str {
        "capabilities"
    }

    /// Default LLM alias for this plan tier, used as the third fallback in
    /// `LlmRegistry::resolve` (after tenant override and caller-supplied alias).
    pub fn default_alias(&self) -> &'static str {
        match self {
            PlanTier::Free => "haiku",
            PlanTier::Pro | PlanTier::Enterprise => "opus",
        }
    }
}

impl std::fmt::Display for PlanTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlanTier::Free => write!(f, "free"),
            PlanTier::Pro => write!(f, "pro"),
            PlanTier::Enterprise => write!(f, "enterprise"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TenantContext {
    pub tenant_id: TenantId,
    pub user_id: Option<UserId>,
    pub plan: PlanTier,
    pub role: UserRole,
    pub workspace_root: PathBuf,
    /// Optional LLM alias or concrete model id that overrides the registry default
    /// for this tenant (e.g. set from DB tenant config or JWT claims).
    /// Resolution order: this field → caller alias → plan default → global default.
    pub preferred_model: Option<String>,
}

impl TenantContext {
    pub fn new(
        tenant_id: impl Into<TenantId>,
        user_id: Option<impl Into<UserId>>,
        plan: PlanTier,
        workspace_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            tenant_id: tenant_id.into(),
            user_id: user_id.map(Into::into),
            plan,
            role: UserRole::User,
            workspace_root: workspace_root.into(),
            preferred_model: None,
        }
    }

    /// Workspace root for this tenant: `{workspace_root}/tenants/{tenant_id}`
    pub fn tenant_root(&self) -> PathBuf {
        self.workspace_root.join("tenants").join(&*self.tenant_id)
    }

    /// Safe join of a relative path under this tenant's root.
    pub fn safe_path(&self, rel: &str) -> common::error::Result<PathBuf> {
        let root = self.tenant_root();
        common::path_safety::safe_join(&root, rel)
    }

    /// S3 / MinIO key prefix for this tenant.
    pub fn storage_prefix(&self) -> String {
        format!("tenants/{}/", &*self.tenant_id)
    }

    /// Storage namespace prefix for this tenant (e.g. Postgres table prefix).
    pub fn tenant_namespace(&self, kind: &str) -> String {
        format!("{}_{}", kind, &*self.tenant_id)
    }

    /// Default system prompt injected into every agent turn.
    pub fn system_prompt(&self) -> String {
        format!(
            "You are a helpful AI assistant for tenant {}. Plan tier: {}.",
            &*self.tenant_id, self.plan
        )
    }

    /// Tracing fields to attach to every span.
    pub fn span_fields(&self) -> Vec<(&'static str, String)> {
        let mut fields = vec![("tenant_id", self.tenant_id.to_string())];
        if let Some(uid) = &self.user_id {
            fields.push(("user_id", uid.to_string()));
        }
        fields.push(("plan", self.plan.to_string()));
        fields
    }
}

/// JWT claims issued by the gateway.
#[derive(Debug, Serialize, Deserialize)]
pub struct TenantClaims {
    pub sub: String, // user_id
    pub tenant_id: String,
    pub plan: PlanTier,
    #[serde(default)]
    pub role: UserRole,
    pub exp: u64,
}
