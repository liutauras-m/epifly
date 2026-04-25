use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
            PlanTier::Free       => 4_096,
            PlanTier::Pro        => 16_384,
            PlanTier::Enterprise => 128_000,
        }
    }

    /// Requests per minute
    pub fn rate_limit_rpm(&self) -> u32 {
        match self {
            PlanTier::Free       => 10,
            PlanTier::Pro        => 60,
            PlanTier::Enterprise => 600,
        }
    }

    pub fn qdrant_collection_prefix(&self) -> &'static str {
        "capabilities"
    }
}

impl std::fmt::Display for PlanTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlanTier::Free       => write!(f, "free"),
            PlanTier::Pro        => write!(f, "pro"),
            PlanTier::Enterprise => write!(f, "enterprise"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TenantContext {
    pub tenant_id: String,
    pub user_id: Option<String>,
    pub plan: PlanTier,
    pub workspace_root: PathBuf,
}

impl TenantContext {
    pub fn new(
        tenant_id: impl Into<String>,
        user_id: Option<String>,
        plan: PlanTier,
        workspace_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            tenant_id: tenant_id.into(),
            user_id,
            plan,
            workspace_root: workspace_root.into(),
        }
    }

    /// Workspace root for this tenant: `{workspace_root}/tenants/{tenant_id}`
    pub fn tenant_root(&self) -> PathBuf {
        self.workspace_root.join("tenants").join(&self.tenant_id)
    }

    /// Safe join of a relative path under this tenant's root.
    pub fn safe_path(&self, rel: &str) -> common::error::Result<PathBuf> {
        let root = self.tenant_root();
        common::path_safety::safe_join(&root, rel)
    }

    /// S3 / MinIO key prefix for this tenant.
    pub fn storage_prefix(&self) -> String {
        format!("tenants/{}/", self.tenant_id)
    }

    /// Qdrant collection name for this tenant.
    pub fn qdrant_collection(&self, kind: &str) -> String {
        format!("{}_{}", kind, self.tenant_id)
    }

    /// Tracing fields to attach to every span.
    pub fn span_fields(&self) -> Vec<(&'static str, String)> {
        let mut fields = vec![("tenant_id", self.tenant_id.clone())];
        if let Some(uid) = &self.user_id {
            fields.push(("user_id", uid.clone()));
        }
        fields.push(("plan", self.plan.to_string()));
        fields
    }
}

/// JWT claims issued by the gateway.
#[derive(Debug, Serialize, Deserialize)]
pub struct TenantClaims {
    pub sub: String,          // user_id
    pub tenant_id: String,
    pub plan: PlanTier,
    pub exp: u64,
}
