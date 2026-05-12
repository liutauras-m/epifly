/// Legacy HMAC/JWT identity provider — wraps the existing auth path so the
/// `IdentityProvider` trait works for both legacy and Zitadel code paths.
use super::{AuthError, IdentityContext, TenantCreated, TenantManager, TenantSummary};
use crate::context::tenant::{PlanTier, SubscriptionStatus, TenantClaims, UserRole};
use crate::identity::IdentityProvider;
use common::types::TenantId;
use async_trait::async_trait;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};

pub struct LegacyIdentityProvider {
    jwt_secret: String,
    dev_secret: String,
}

impl LegacyIdentityProvider {
    pub fn from_env() -> Self {
        Self {
            jwt_secret: std::env::var("JWT_SECRET").unwrap_or_default(),
            dev_secret: "conusai-dev-secret-not-for-production".into(),
        }
    }

    fn signing_key(&self) -> &str {
        if self.jwt_secret.is_empty() {
            &self.dev_secret
        } else {
            &self.jwt_secret
        }
    }
}

#[async_trait]
impl IdentityProvider for LegacyIdentityProvider {
    async fn verify_access_token(&self, token: &str) -> Result<IdentityContext, AuthError> {
        let key = DecodingKey::from_secret(self.signing_key().as_bytes());
        let claims = decode::<TenantClaims>(token, &key, &Validation::new(Algorithm::HS256))
            .map_err(|e| AuthError::InvalidToken(e.to_string()))?
            .claims;

        Ok(IdentityContext {
            user_id: claims.sub.clone(),
            tenant_id: TenantId::from(claims.tenant_id),
            email: Some(claims.sub),
            roles: vec![claims.role],
            plan_tier: claims.plan,
            subscription_status: claims.subscription_status,
        })
    }

    async fn user_info(&self, sub: &str) -> Result<IdentityContext, AuthError> {
        // Legacy provider has no user store — return minimal context.
        Ok(IdentityContext {
            user_id: sub.to_string(),
            tenant_id: TenantId::from(sub.to_string()),
            email: Some(sub.to_string()),
            roles: vec![UserRole::User],
            plan_tier: PlanTier::Free,
            subscription_status: SubscriptionStatus::Active,
        })
    }

    async fn health(&self) -> Result<(), AuthError> {
        Ok(())
    }
}

#[async_trait]
impl TenantManager for LegacyIdentityProvider {
    async fn create_tenant(
        &self,
        name: &str,
        owner_email: &str,
    ) -> Result<TenantCreated, AuthError> {
        Ok(TenantCreated {
            tenant_id: TenantId::from(owner_email.to_string()),
            name: name.to_string(),
            owner_email: owner_email.to_string(),
        })
    }

    async fn list_tenants(&self) -> Result<Vec<TenantSummary>, AuthError> {
        Ok(vec![])
    }

    async fn invite_user(
        &self,
        _tenant_id: &TenantId,
        _email: &str,
        _role: UserRole,
    ) -> Result<(), AuthError> {
        Ok(())
    }

    async fn update_plan_claim(
        &self,
        _tenant_id: &TenantId,
        _tier: PlanTier,
        _status: SubscriptionStatus,
    ) -> Result<(), AuthError> {
        Ok(())
    }

    async fn health(&self) -> Result<(), AuthError> {
        Ok(())
    }
}
