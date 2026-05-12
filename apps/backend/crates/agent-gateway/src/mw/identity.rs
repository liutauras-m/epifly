/// Identity resolution middleware.
///
/// When `CONUSAI_AUTH_PROVIDER=zitadel`, calls ZitadelProvider to verify the
/// Bearer access token and inserts `ResolvedTenant` + `ResolvedIdentity` into
/// request extensions.
///
/// When `CONUSAI_AUTH_PROVIDER=legacy` (or unset), falls through to the existing
/// `mw::tenant::extract_tenant` code path so legacy HMAC/JWT auth is unaffected.
use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use agent_core::identity::{IdentityContext, IdentityProvider as _};
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
};
use common::error::HttpError;
use std::sync::Arc;
use tracing::warn;

/// Extension key for the full `IdentityContext` (Zitadel mode only).
#[derive(Clone)]
pub struct ResolvedIdentity(pub IdentityContext);

pub async fn extract_identity(
    State(state): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Response {
    // API-key middleware already resolved tenant — skip.
    if req.extensions().get::<ResolvedTenant>().is_some() {
        return next.run(req).await;
    }

    let provider = std::env::var("CONUSAI_AUTH_PROVIDER").unwrap_or_else(|_| "legacy".into());

    if provider != "zitadel" {
        // Legacy path — handled by extract_tenant further down the stack.
        return next.run(req).await;
    }

    let token = bearer_token(&req);
    let Some(token) = token else {
        return HttpError::auth("authentication required").into_response();
    };

    match state.identity.verify_access_token(token).await {
        Ok(identity) => {
            let workspace_root = std::env::var("CONUSAI_WORKSPACE_ROOT")
                .unwrap_or_else(|_| "/tmp/conusai/workspaces".into());
            let tenant_ctx = identity.clone().into_tenant_context(workspace_root);
            req.extensions_mut().insert(ResolvedTenant(tenant_ctx));
            req.extensions_mut().insert(ResolvedIdentity(identity));
            next.run(req).await
        }
        Err(e) => {
            warn!(error = %e, "identity verification failed");
            HttpError::auth("invalid or expired token").into_response()
        }
    }
}

fn bearer_token(req: &Request) -> Option<&str> {
    req.headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
}
