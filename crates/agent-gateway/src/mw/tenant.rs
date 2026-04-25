use agent_core::{TenantContext, TenantClaims, PlanTier};
use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use std::sync::Arc;
use tracing::warn;
use crate::state::AppState;

/// Axum extension key for the resolved tenant.
#[derive(Clone)]
pub struct ResolvedTenant(pub TenantContext);

/// Middleware: extract tenant from JWT Bearer token or `X-Tenant-ID` header.
/// - If a valid JWT is present → decode claims → build TenantContext
/// - If only `X-Tenant-ID` header → dev mode (free tier, no auth)
/// - If neither → reject with 401
pub async fn extract_tenant(
    State(state): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Response {
    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_default();

    // Try Authorization: Bearer <jwt>
    if let Some(token) = bearer_token(&req) {
        if !jwt_secret.is_empty() {
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
                        std::env::var("CONUSAI_WORKSPACE_ROOT")
                            .unwrap_or_else(|_| "/tmp/conusai/workspaces".into()),
                    );
                    req.extensions_mut().insert(ResolvedTenant(tenant));
                    return next.run(req).await;
                }
                Err(e) => {
                    warn!(error = %e, "JWT decode failed");
                    return (StatusCode::UNAUTHORIZED, "invalid token").into_response();
                }
            }
        }
    }

    // Dev fallback: X-Tenant-ID header (no auth check)
    if let Some(tid) = req.headers()
        .get("X-Tenant-ID")
        .and_then(|v| v.to_str().ok())
    {
        let tenant = TenantContext::new(
            tid,
            None,
            PlanTier::Free,
            std::env::var("CONUSAI_WORKSPACE_ROOT")
                .unwrap_or_else(|_| "/tmp/conusai/workspaces".into()),
        );
        req.extensions_mut().insert(ResolvedTenant(tenant));
        return next.run(req).await;
    }

    // No auth in dev mode (JWT_SECRET not set) → default tenant
    if jwt_secret.is_empty() {
        let tenant = TenantContext::new(
            "dev",
            None,
            PlanTier::Enterprise,
            std::env::var("CONUSAI_WORKSPACE_ROOT")
                .unwrap_or_else(|_| "/tmp/conusai/workspaces".into()),
        );
        req.extensions_mut().insert(ResolvedTenant(tenant));
        return next.run(req).await;
    }

    (StatusCode::UNAUTHORIZED, "authentication required").into_response()
}

fn bearer_token(req: &Request) -> Option<&str> {
    req.headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
}
