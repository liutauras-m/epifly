//! Caller identity.
//!
//! Production identity is Zitadel only: an access token is verified by a
//! [`TokenVerifier`] and yields an [`Identity`]. The tenant is the Zitadel
//! organization that owns the user; the plan tier is never taken from a token
//! (the gateway resolves it server-side from billing).

pub mod zitadel;

use crate::context::tenant::UserRole;
use async_trait::async_trait;
use common::types::TenantId;
use thiserror::Error;

pub use zitadel::{ZitadelConfig, ZitadelVerifier};

#[derive(Debug, Error)]
pub enum AuthError {
    /// The credential is malformed, expired, revoked or not meant for us.
    #[error("invalid token: {0}")]
    InvalidToken(String),

    /// The identity provider could not be reached or answered unexpectedly.
    #[error("identity provider unavailable: {0}")]
    Unavailable(String),
}

/// A verified caller.
#[derive(Debug, Clone, PartialEq)]
pub struct Identity {
    /// Zitadel user id (`sub`).
    pub user_id: String,
    /// Zitadel organization id (`urn:zitadel:iam:user:resourceowner:id`).
    pub tenant_id: TenantId,
    /// Verified email, display only — never used for authorization.
    pub email: Option<String>,
    /// Highest platform role granted through the project roles claim.
    pub role: UserRole,
}

/// Verifies a bearer access token.
#[async_trait]
pub trait TokenVerifier: Send + Sync + 'static {
    async fn verify(&self, token: &str) -> Result<Identity, AuthError>;
}

/// Map Zitadel project role keys to the platform role.
/// `super_admin` → [`UserRole::SuperAdmin`], `admin` → [`UserRole::Admin`].
pub fn role_from_project_roles<'a>(keys: impl IntoIterator<Item = &'a str>) -> UserRole {
    let mut role = UserRole::User;
    for k in keys {
        match k {
            "super_admin" => return UserRole::SuperAdmin,
            "admin" => role = UserRole::Admin,
            _ => {}
        }
    }
    role
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_mapping() {
        assert_eq!(role_from_project_roles([]), UserRole::User);
        assert_eq!(role_from_project_roles(["member"]), UserRole::User);
        assert_eq!(role_from_project_roles(["admin"]), UserRole::Admin);
        assert_eq!(
            role_from_project_roles(["admin", "super_admin"]),
            UserRole::SuperAdmin
        );
    }
}
