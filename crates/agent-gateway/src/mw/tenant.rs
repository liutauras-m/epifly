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

/// Middleware: extract tenant from JWT Bearer token or HMAC session cookie.
///
/// Behavior depends on whether `JWT_SECRET` is set:
///
/// **Production mode (`JWT_SECRET` set)**:
///   1. Valid HS256 Bearer JWT → accepted (API clients, CI).
///   2. Valid `conusai_session` HMAC cookie → accepted (SvelteKit SSR calls to
///      `/v1/*`, e.g. loading Recents and Capabilities on page load).
///   3. No credential → 401.
///
/// This dual-credential policy lets the SvelteKit web app call `/v1/*` from
/// SSR without issuing a separate JWT at login time.
///
/// **Dev mode (`JWT_SECRET` unset)**: `X-Tenant-ID` header accepted as a
/// no-auth tenant claim; falls back to session cookie; then to a default `dev`
/// tenant. This mode must never run in production.
pub async fn extract_tenant(
    State(_state): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Response {
    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_default();
    let workspace_root = std::env::var("CONUSAI_WORKSPACE_ROOT")
        .unwrap_or_else(|_| "/tmp/conusai/workspaces".into());

    if !jwt_secret.is_empty() {
        // ── Production mode: try JWT Bearer first, then fall back to session cookie.
        if let Some(token) = bearer_token(&req) {
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
                    return next.run(req).await;
                }
                Err(e) => {
                    warn!(error = %e, "JWT decode failed");
                    return (StatusCode::UNAUTHORIZED, "invalid token").into_response();
                }
            }
        }

        // No Bearer token — try the HMAC session cookie (SvelteKit SSR).
        if let Some(user) = session_cookie(&req) {
            let tenant = user.tenant_context();
            req.extensions_mut().insert(ResolvedTenant(tenant));
            return next.run(req).await;
        }

        (StatusCode::UNAUTHORIZED, "authentication required").into_response()
    } else {
        // ── Dev mode: X-Tenant-ID header, then session cookie, then default.
        let header_tid = req
            .headers()
            .get("X-Tenant-ID")
            .and_then(|v| v.to_str().ok())
            .map(String::from);

        let tenant = if let Some(tid) = header_tid {
            TenantContext::new(tid, None, PlanTier::Free, workspace_root)
        } else if let Some(user) = session_cookie(&req) {
            user.tenant_context()
        } else {
            TenantContext::new("dev", None, PlanTier::Enterprise, workspace_root)
        };
        req.extensions_mut().insert(ResolvedTenant(tenant));
        next.run(req).await
    }
}

fn session_cookie(req: &Request) -> Option<crate::ui::session::SessionUser> {
    req.headers()
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|c| {
                let c = c.trim();
                c.strip_prefix("conusai_session=")
                    .and_then(crate::ui::session::verify)
            })
        })
}

fn bearer_token(req: &Request) -> Option<&str> {
    req.headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
}
