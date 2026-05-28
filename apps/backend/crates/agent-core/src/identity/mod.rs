pub mod binding;
pub mod legacy;
pub mod zitadel;

use crate::context::tenant::{PlanTier, SubscriptionStatus, TenantContext, UserRole};
use async_trait::async_trait;
use common::types::TenantId;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ── AuthError ─────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("invalid token: {0}")]
    InvalidToken(String),

    #[error("token expired")]
    TokenExpired,

    #[error("authentication required")]
    Unauthenticated,

    #[error("tenant not found: {0}")]
    TenantNotFound(String),

    #[error("provider error: {0}")]
    Provider(String),

    #[error("configuration error: {0}")]
    Config(String),
}

// ── IdentityContext ───────────────────────────────────────────────────────────

/// Resolved identity from any auth provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityContext {
    pub user_id: String,
    pub tenant_id: TenantId,
    pub email: Option<String>,
    pub roles: Vec<UserRole>,
    pub plan_tier: PlanTier,
    pub subscription_status: SubscriptionStatus,
}

impl IdentityContext {
    /// Convert to a `TenantContext` for back-compat with existing middleware.
    pub fn into_tenant_context(
        self,
        workspace_root: impl Into<std::path::PathBuf>,
    ) -> TenantContext {
        let role = self.roles.first().cloned().unwrap_or(UserRole::User);
        let mut ctx = TenantContext::new(
            self.tenant_id,
            Some(self.user_id),
            self.plan_tier,
            workspace_root,
        );
        ctx.role = role;
        ctx
    }
}

// ── Tenant admin types ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantCreated {
    pub tenant_id: TenantId,
    pub name: String,
    pub owner_email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantSummary {
    pub tenant_id: TenantId,
    pub name: String,
    pub owner_email: Option<String>,
    pub plan_tier: PlanTier,
    pub subscription_status: SubscriptionStatus,
}

// ── IdentityProvider ──────────────────────────────────────────────────────────

#[async_trait]
pub trait IdentityProvider: Send + Sync + 'static {
    async fn verify_access_token(&self, token: &str) -> Result<IdentityContext, AuthError>;
    async fn user_info(&self, sub: &str) -> Result<IdentityContext, AuthError>;
    async fn health(&self) -> Result<(), AuthError>;
}

/// Combined supertrait — every identity manager can also verify tokens.
pub trait IdentityManager: IdentityProvider + TenantManager {}
impl<T: IdentityProvider + TenantManager> IdentityManager for T {}

// ── TenantManager ─────────────────────────────────────────────────────────────

#[async_trait]
pub trait TenantManager: Send + Sync + 'static {
    async fn create_tenant(
        &self,
        name: &str,
        owner_email: &str,
    ) -> Result<TenantCreated, AuthError>;
    async fn list_tenants(&self) -> Result<Vec<TenantSummary>, AuthError>;
    async fn invite_user(
        &self,
        tenant_id: &TenantId,
        email: &str,
        role: UserRole,
    ) -> Result<(), AuthError>;
    async fn update_plan_claim(
        &self,
        tenant_id: &TenantId,
        tier: PlanTier,
        status: SubscriptionStatus,
    ) -> Result<(), AuthError>;
    async fn health(&self) -> Result<(), AuthError>;
}
