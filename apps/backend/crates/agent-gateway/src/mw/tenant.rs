use crate::auth::audit;
use crate::state::AppState;
use agent_core::identity::binding;
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
        && (req.headers().contains_key("x-tenant-id") || req.headers().contains_key("X-Tenant-ID"))
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
            Ok(mut identity_ctx) => {
                // Phase 6: resolve tenant via binding table when Postgres is configured.
                if let Some(db) = &state.db {
                    let issuer = std::env::var("ZITADEL_ISSUER").unwrap_or_default();
                    let org_id = identity_ctx.tenant_id.as_ref().to_string();
                    let sub = &identity_ctx.user_id;
                    match binding::resolve_tenant(db, &issuer, &org_id, sub).await {
                        Ok(b) => {
                            // Replace org_id with application tenant_id from binding
                            let resolved_sub = sub.to_string();
                            audit::login_success(&issuer, &resolved_sub, &org_id);
                            identity_ctx.tenant_id = b.tenant_id.into();
                        }
                        Err(binding::BindingError::NotProvisioned(ref org)) => {
                            warn!(request_id, org_id = %org, "tenant not provisioned");
                            audit::tenant_binding_failure(&issuer, org, "not_provisioned");
                            return HttpError::forbidden("tenant_not_provisioned").into_response();
                        }
                        Err(binding::BindingError::Suspended(ref tid)) => {
                            warn!(request_id, tenant_id = %tid, "tenant suspended");
                            audit::tenant_binding_failure(&issuer, &org_id, "suspended");
                            return HttpError::forbidden("tenant_suspended").into_response();
                        }
                        Err(e) => {
                            warn!(request_id, error = %e, "binding lookup error");
                            return HttpError::internal("binding_error", None).into_response();
                        }
                    }
                }
                let tenant = identity_ctx.into_tenant_context(&state.workspace_root);
                req.extensions_mut().insert(ResolvedTenant(tenant));
                return next.run(req).await;
            }
            Err(e) => {
                if state.auth_required {
                    warn!(
                        request_id,
                        error = %e,
                        auth.outcome = "invalid_token",
                        "bearer token rejected"
                    );
                    audit::login_failure("invalid_token");
                    return HttpError::auth("invalid token").into_response();
                }
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
    //    Only active when ZITADEL_ISSUER is not configured.
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::body::Body;
    use axum::http::StatusCode;
    use axum::middleware::from_fn_with_state;
    use axum::routing::get;
    use tower::ServiceExt;

    fn prod_state() -> Arc<AppState> {
        let mut state = AppState::with_in_memory_stores().expect("in-memory AppState for test");
        state.auth_required = true;
        Arc::new(state)
    }

    fn dev_state() -> Arc<AppState> {
        let mut state = AppState::with_in_memory_stores().expect("in-memory AppState for test");
        state.auth_required = false;
        Arc::new(state)
    }

    async fn ok_handler() -> StatusCode {
        StatusCode::OK
    }

    /// Sending `X-Tenant-ID` header in prod mode (auth_required=true) must return
    /// `400 tenant_header_forbidden` — regardless of whether the token is valid.
    /// This is the `tenant_header_rejected_in_prod` CI gate.
    #[tokio::test]
    async fn tenant_header_rejected_in_prod() {
        let state = prod_state();
        let app = Router::new()
            .route("/", get(ok_handler))
            .layer(from_fn_with_state(Arc::clone(&state), extract_tenant))
            .with_state(state);

        let req = axum::http::Request::builder()
            .uri("/")
            .header("X-Tenant-ID", "tenant-b")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::BAD_REQUEST,
            "X-Tenant-ID must be rejected with 400 in prod mode"
        );
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = std::str::from_utf8(&body).unwrap();
        assert!(
            body_str.contains("tenant_header_forbidden"),
            "error_code must be tenant_header_forbidden, got: {body_str}"
        );
    }

    /// Lowercase `x-tenant-id` is equally forbidden in prod mode (HTTP headers are
    /// case-insensitive; Axum normalises them to lowercase by default).
    #[tokio::test]
    async fn tenant_header_lowercase_rejected_in_prod() {
        let state = prod_state();
        let app = Router::new()
            .route("/", get(ok_handler))
            .layer(from_fn_with_state(Arc::clone(&state), extract_tenant))
            .with_state(state);

        let req = axum::http::Request::builder()
            .uri("/")
            .header("x-tenant-id", "tenant-b")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::BAD_REQUEST,
            "lowercase x-tenant-id must be rejected with 400 in prod mode"
        );
    }

    /// In dev mode (auth_required=false) the `X-Tenant-ID` header is honoured and
    /// the resolved tenant id is forwarded via the `ResolvedTenant` extension.
    #[tokio::test]
    async fn tenant_header_honored_in_dev() {
        let state = dev_state();

        async fn tenant_echo(req: axum::extract::Request) -> (StatusCode, String) {
            match req.extensions().get::<ResolvedTenant>().cloned() {
                Some(ResolvedTenant(ctx)) => (StatusCode::OK, ctx.tenant_id.to_string()),
                None => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "missing tenant ext".into(),
                ),
            }
        }

        let app = Router::new()
            .route("/", get(tenant_echo))
            .layer(from_fn_with_state(Arc::clone(&state), extract_tenant))
            .with_state(state);

        let req = axum::http::Request::builder()
            .uri("/")
            .header("X-Tenant-ID", "dev-tenant-xyz")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(std::str::from_utf8(&body).unwrap(), "dev-tenant-xyz");
    }
}
