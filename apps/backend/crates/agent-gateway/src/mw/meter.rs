/// Usage metering middleware — runs AFTER the handler.
///
/// Reads `AgentTurnStats` from response extensions (populated by agent/chat
/// handlers) and calls `billing.report_usage` + `quota.record`.
use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use billing_core::events::{ActionType, UsageEvent};
use std::sync::Arc;
use tracing::warn;

/// Stats inserted by agent/chat handlers into response extensions.
#[derive(Debug, Clone)]
pub struct AgentTurnStats {
    pub tokens: u64,
    pub turns: u32,
    pub model: String,
    pub duration_ms: u64,
}

pub async fn record_usage(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    // Capture tenant before consuming the request.
    let tenant = req
        .extensions()
        .get::<ResolvedTenant>()
        .cloned();

    let resp = next.run(req).await;

    // Only meter if we have a resolved tenant and billing is configured.
    let Some(ResolvedTenant(ctx)) = tenant else {
        return resp;
    };
    let Some(billing) = &state.billing else {
        return resp;
    };
    let Some(quota) = &state.quota else {
        return resp;
    };

    // Read AgentTurnStats if the handler populated it.
    if let Some(stats) = resp.extensions().get::<AgentTurnStats>().cloned() {
        let lago_customer_id = ctx.tenant_id.to_string();

        // Report agent turn.
        let turn_event = UsageEvent::new(
            ctx.tenant_id.to_string(),
            lago_customer_id.clone(),
            ActionType::AgentTurn,
            stats.turns as u64,
        )
        .with_properties(serde_json::json!({
            "model": stats.model,
            "duration_ms": stats.duration_ms,
        }));

        if let Err(e) = billing.report_usage(turn_event).await {
            warn!(error = %e, "metering: report_usage(agent_turn) failed");
        }
        quota
            .record(&ctx.tenant_id, &ActionType::AgentTurn, stats.turns as u64)
            .await;

        // Report token usage.
        if stats.tokens > 0 {
            let token_event = UsageEvent::new(
                ctx.tenant_id.to_string(),
                lago_customer_id,
                ActionType::Token,
                stats.tokens,
            );
            if let Err(e) = billing.report_usage(token_event).await {
                warn!(error = %e, "metering: report_usage(token) failed");
            }
            quota
                .record(&ctx.tenant_id, &ActionType::Token, stats.tokens)
                .await;
        }
    }

    resp
}
