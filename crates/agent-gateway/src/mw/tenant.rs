use crate::state::AppState;
use agent_core::{PlanTier, TenantClaims, TenantContext};
use axum::{
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use std::sync::Arc;
use tracing::warn;

/// Axum extension key for the resolved tenant.
#[derive(Clone)]
pub struct ResolvedTenant(pub TenantContext);

/// Middleware: extract tenant from JWT Bearer token.
///
/// Behavior depends on whether `JWT_SECRET` is set:
///
/// **Production mode (`JWT_SECRET` set)**: a valid HS256 Bearer JWT is REQUIRED.
/// `X-Tenant-ID` is ignored. Missing or invalid token → 401.
///
/// **Dev mode (`JWT_SECRET` unset)**: `X-Tenant-ID` header is accepted as a
/// no-auth tenant claim. With no header, requests fall through to a default
/// `dev` tenant. This mode must never run in production.
pub async fn extract_tenant(
    State(_state): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Response {
    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_default();
    let workspace_root = std::env::var("CONUSAI_WORKSPACE_ROOT")
        .unwrap_or_else(|_| "/tmp/conusai/workspaces".into());

    // ── Production mode: JWT_SECRET set → JWT is the ONLY accepted credential.
    if !jwt_secret.is_empty() {
        let Some(token) = bearer_token(&req) else {
            return (StatusCode::UNAUTHORIZED, "authentication required").into_response();
        };
        match decode::<TenantClaims>(
            token,
            &DecodingKey::from_secret(jwt_secret.as_bytes()),
            &Validation::new(Algorithm::HS256),
        ) {
            Ok(data) => {
                let claims = data.claims;
                let tenant = TenantContext::new(
                    &claims.tenant_id,
                    Some(claims.sub),
                    claims.plan,
                    workspace_root,
                );
                req.extensions_mut().insert(ResolvedTenant(tenant));
                next.run(req).await
            }
            Err(e) => {
                warn!(error = %e, "JWT decode failed");
                (StatusCode::UNAUTHORIZED, "invalid token").into_response()
            }
        }
    } else {
        // ── Dev mode: JWT_SECRET unset. Accept X-Tenant-ID, otherwise default.
        let tenant = if let Some(tid) = req
            .headers()
            .get("X-Tenant-ID")
            .and_then(|v| v.to_str().ok())
        {
            TenantContext::new(tid, None, PlanTier::Free, workspace_root)
        } else {
            TenantContext::new("dev", None, PlanTier::Enterprise, workspace_root)
        };
        req.extensions_mut().insert(ResolvedTenant(tenant));
        next.run(req).await
    }
}

fn bearer_token(req: &Request) -> Option<&str> {
    req.headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
}
