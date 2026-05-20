//! Middleware that enforces the `super_admin` role on admin routes.
//!
//! For JWT-authed `/admin/*` routes: reads `TenantClaims.role`.
//! For UI `/super-admin/*` routes: reads `SessionUser.role`.

use crate::mw::tenant::ResolvedTenant;
use axum::{
    Extension,
    body::Body,
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};

/// Middleware for API routes (JWT): requires `role = SuperAdmin` in claims.
pub async fn require_super_admin_jwt(
    Extension(tenant): Extension<ResolvedTenant>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    use agent_core::UserRole;
    if tenant.0.role != UserRole::SuperAdmin {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(next.run(req).await)
}

