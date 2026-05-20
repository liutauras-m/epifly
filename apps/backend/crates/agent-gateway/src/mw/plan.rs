/// Plan enforcement middleware.
///
/// Reads `ResolvedTenant` from extensions, validates the plan tier, and inserts
/// `Extension<PlanLimits>` so handlers can clamp tokens/turns/RPM without
/// calling the deprecated `PlanTier::max_tokens()` etc.
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

/// Middleware: validate plan and insert `Extension<PlanLimits>` for handlers.
pub async fn enforce_plan(mut req: Request, next: Next) -> Response {
    let tenant = req.extensions().get::<ResolvedTenant>().cloned();

    match tenant {
        None => {
            warn!("enforce_plan: no ResolvedTenant extension found — rejecting");
            HttpError::auth("authentication required").into_response()
        }
        Some(t) => {
            // Validate plan tier is recognized.
            match &t.0.plan {
                PlanTier::Free | PlanTier::Pro | PlanTier::Enterprise => {}
            }
            let limits = t.0.plan.limits();
            req.extensions_mut().insert(limits);
            next.run(req).await
        }
    }
}
