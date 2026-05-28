use crate::state::AppState;
use agent_core::{PlanTier, TenantContext};
use axum::{
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::{IntoResponse, Response},
};
use common::error::HttpError;
use std::sync::Arc;
use tracing::warn;

/// Axum extension key for the resolved tenant context.
#[derive(Clone)]
pub struct ResolvedTenant(pub TenantContext);

/// Middleware: resolve a [`ResolvedTenant`] from the incoming request.
///
/// Auth vectors in priority order:
///
/// 1. **API key** (`X-API-Key` header) — already resolved by `api_key` middleware; skip.
/// 2. **Bearer JWT** (`Authorization: Bearer <token>`) — verified by `state.identity`.
///    Works for both Zitadel RS256 tokens and legacy HS256 JWTs.
/// 3. **Cookie / `X-Session-Token`** — legacy web + Tauri WKWebView HMAC path.
/// 4. **Dev fallback** (only when `!state.auth_required`) — `X-Tenant-ID` header or
///    default `dev` / `Enterprise` tenant.  Never active in production.
pub async fn extract_tenant(
    State(state): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Response {
    // API-key middleware already resolved the tenant — skip all session auth.
    if req.extensions().get::<ResolvedTenant>().is_some() {
        return next.run(req).await;
    }

    // In OIDC/prod mode, reject any attempt to inject a tenant via header —
    // clients must use a real bearer token.
    if state.auth_required
        && (req.headers().contains_key("x-tenant-id")
            || req.headers().contains_key("X-Tenant-ID"))
    {
        return HttpError::bad_request("tenant_header_forbidden").into_response();
    }

    let request_id = req
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-");

    // 1. Bearer token → unified identity verification (Zitadel RS256 or Legacy HS256).
    if let Some(token) = bearer_token(&req) {
        match state.identity.verify_access_token(token).await {
            Ok(identity_ctx) => {
                let tenant = identity_ctx.into_tenant_context(&state.workspace_root);
                req.extensions_mut().insert(ResolvedTenant(tenant));
                return next.run(req).await;
            }
            Err(e) => {
                if state.auth_required {
                    // Production / Zitadel mode: invalid token → 401.
                    warn!(
                        request_id,
                        error = %e,
                        auth.outcome = "invalid_token",
                        "bearer token rejected"
                    );
                    return HttpError::auth("invalid token").into_response();
                }
                // Dev mode: log and fall through to X-Tenant-ID / cookie / default.
                warn!(
                    request_id,
                    error = %e,
                    auth.outcome = "dev_token_fallthrough",
                    "dev mode: bearer token invalid, continuing without bearer auth"
                );
            }
        }
    }

    // 2. Cookie / X-Session-Token (web + Tauri WKWebView legacy HMAC path).
    if let Some(user) = crate::auth::extract_from_headers(req.headers()) {
        req.extensions_mut()
            .insert(ResolvedTenant(user.tenant_context()));
        return next.run(req).await;
    }

    // 3. Dev-mode fallback: X-Tenant-ID header or default "dev" tenant.
    //    Only active when neither JWT_SECRET nor ZITADEL_DOMAIN is configured.
    if !state.auth_required {
        let header_tid = req
            .headers()
            .get("X-Tenant-ID")
            .and_then(|v| v.to_str().ok())
            .map(String::from);

        let tenant = if let Some(tid) = header_tid {
            TenantContext::new(tid, None::<&str>, PlanTier::Free, &state.workspace_root)
        } else {
            TenantContext::new(
                "dev",
                None::<&str>,
                PlanTier::Enterprise,
                &state.workspace_root,
            )
        };
        req.extensions_mut().insert(ResolvedTenant(tenant));
        return next.run(req).await;
    }

    warn!(
        request_id,
        auth.outcome = "unauthenticated",
        "no credentials presented"
    );
    HttpError::auth("authentication required").into_response()
}

fn bearer_token(req: &Request) -> Option<&str> {
    req.headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
}
