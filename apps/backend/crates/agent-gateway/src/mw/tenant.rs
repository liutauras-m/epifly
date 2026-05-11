use crate::state::AppState;
use agent_core::{PlanTier, TenantClaims, TenantContext};
use axum::{
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::{IntoResponse, Response},
};
use common::error::HttpError;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use std::sync::Arc;
use tracing::warn;

/// Axum extension key for the resolved tenant context.
#[derive(Clone)]
pub struct ResolvedTenant(pub TenantContext);

/// Middleware: resolve a [`ResolvedTenant`] from the incoming request.
///
/// Auth vectors in priority order:
///
/// **Production mode (`JWT_SECRET` set)**
///   1. `Authorization: Bearer <HS256-JWT>` — external API / machine-to-machine.
///   2. `conusai_session` cookie — web app, same-origin.
///   3. `X-Session-Token` header — Tauri WKWebView (cannot attach Secure cookies
///      to cross-origin HTTP; browser-shell injects the HMAC token as a header).
///   4. Anything else → 401.
///
/// **Dev mode (`JWT_SECRET` unset)**
///   1. `X-Tenant-ID` header — bare tenant override, no auth check.
///   2. Session cookie or `X-Session-Token` header (via [`crate::auth`]).
///   3. Default `dev` tenant (no auth required).
pub async fn extract_tenant(
    State(_state): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Response {
    // API-key middleware already resolved the tenant — skip all session auth.
    if req.extensions().get::<ResolvedTenant>().is_some() {
        return next.run(req).await;
    }

    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_default();
    let workspace_root = std::env::var("CONUSAI_WORKSPACE_ROOT")
        .unwrap_or_else(|_| "/tmp/conusai/workspaces".into());

    if !jwt_secret.is_empty() {
        // ── Production mode ───────────────────────────────────────────────

        // 1. Bearer JWT (external API clients / machine-to-machine).
        if let Some(token) = bearer_token(&req) {
            return match decode::<TenantClaims>(
                token,
                &DecodingKey::from_secret(jwt_secret.as_bytes()),
                &Validation::new(Algorithm::HS256),
            ) {
                Ok(data) => {
                    let claims = data.claims;
                    let mut tenant = TenantContext::new(
                        claims.tenant_id.as_str(),
                        Some(claims.sub),
                        claims.plan,
                        workspace_root,
                    );
                    tenant.role = claims.role;
                    req.extensions_mut().insert(ResolvedTenant(tenant));
                    next.run(req).await
                }
                Err(e) => {
                    warn!(error = %e, "JWT decode failed");
                    HttpError::auth("invalid token").into_response()
                }
            };
        }

        // 2 & 3. Cookie or X-Session-Token header (web + Tauri WKWebView).
        if let Some(user) = crate::auth::extract_from_headers(req.headers()) {
            req.extensions_mut()
                .insert(ResolvedTenant(user.tenant_context()));
            return next.run(req).await;
        }

        HttpError::auth("authentication required").into_response()
    } else {
        // ── Dev mode ─────────────────────────────────────────────────────

        let header_tid = req
            .headers()
            .get("X-Tenant-ID")
            .and_then(|v| v.to_str().ok())
            .map(String::from);

        let tenant = if let Some(tid) = header_tid {
            TenantContext::new(tid, None::<&str>, PlanTier::Free, workspace_root)
        } else if let Some(user) = crate::auth::extract_from_headers(req.headers()) {
            user.tenant_context()
        } else {
            TenantContext::new("dev", None::<&str>, PlanTier::Enterprise, workspace_root)
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
