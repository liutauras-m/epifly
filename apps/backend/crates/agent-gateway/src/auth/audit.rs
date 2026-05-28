//! Structured auth audit log.
//!
//! Events are emitted via the `tracing` crate with `target = "audit"`.
//! They are intentionally separate from the application log stream and must
//! NEVER contain: access_token, refresh_token, id_token, code, email,
//! full IP, Authorization header, Cookie header, or raw OIDC claims.

/// Emit `auth.login.success` when a JWT is accepted and a tenant context is resolved.
pub fn login_success(iss: &str, sub: &str, org_id: &str) {
    tracing::info!(
        target: "audit",
        event = "auth.login.success",
        iss = iss,
        sub = sub,
        org_id = org_id,
    );
}

/// Emit `auth.login.failure` when a bearer token is rejected.
pub fn login_failure(reason: &str) {
    tracing::info!(
        target: "audit",
        event = "auth.login.failure",
        reason = reason,
    );
}

/// Emit `auth.tenant_binding.failure` when org_id has no binding or is suspended.
pub fn tenant_binding_failure(iss: &str, org_id: &str, reason: &str) {
    tracing::info!(
        target: "audit",
        event = "auth.tenant_binding.failure",
        iss = iss,
        org_id = org_id,
        reason = reason,
    );
}
