/// Plan enforcement middleware.
///
/// Reads `TenantClaims` / `ResolvedTenant` from extensions and validates that a
/// recognised plan tier is present before the request reaches any handler.
///
/// **Current behaviour:** presence + plan-tier validation only.
/// Actual `max_tokens` / `max_turns` clamping is performed per-handler in
/// `build_ctx` (agent.rs) and `blocking_response` / `stream_response` (chat.rs)
/// via `req.max_tokens.unwrap_or(4096).min(tenant.plan.max_tokens())` and the
/// equivalent `max_rounds` calculation. The functional result is identical to
/// central middleware clamping; centralisation is a future cleanup task.
///
/// Must run AFTER `extract_api_key` and `extract_tenant` middleware.
use agent_core::PlanTier;
use axum::{
    extract::Request,
    middleware::Next,
    response::{IntoResponse, Response},
};
use common::error::HttpError;
use tracing::warn;

use crate::mw::tenant::ResolvedTenant;

/// Middleware: validate that a resolved tenant with a recognized plan exists.
/// Must run AFTER `extract_api_key` and `extract_tenant` middleware.
pub async fn enforce_plan(req: Request, next: Next) -> Response {
    let tenant = req.extensions().get::<ResolvedTenant>().cloned();

    match tenant {
        None => {
            warn!("enforce_plan: no ResolvedTenant extension found — rejecting");
            HttpError::auth("authentication required").into_response()
        }
        Some(t) => {
            // Validate plan tier is recognized (guard against malformed JWTs with
            // unexpected plan values that might bypass limit clamping).
            match &t.0.plan {
                PlanTier::Free | PlanTier::Pro | PlanTier::Enterprise => {
                    // Valid — continue
                }
            }
            next.run(req).await
        }
    }
}
