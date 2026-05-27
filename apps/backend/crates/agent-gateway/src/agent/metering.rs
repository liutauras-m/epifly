//! Agent turn metering — Step 2.4.
//!
//! Moved from `routes/agent.rs`; now called from `AgentTurnRunner` so both
//! blocking and streaming paths emit identical signals.

use crate::state::AppState;
use billing_core::events::{ActionType, UsageEvent};
use common::metrics;
use std::sync::Arc;
use tracing::warn;

/// Record OTel metrics + billing + quota for a completed agent turn.
pub async fn record_agent_usage(
    state: &Arc<AppState>,
    tenant_id: &str,
    model: &str,
    input_tokens: u64,
    output_tokens: u64,
    _tool_calls: usize,
    duration_ms: u64,
) {
    let model_label = [metrics::kv("model", model)];
    metrics::llm_requests().add(1, &model_label);
    metrics::llm_input_tokens().record(input_tokens, &model_label);
    metrics::llm_output_tokens().record(output_tokens, &model_label);

    if let (Some(billing), Some(quota)) = (&state.billing, &state.quota) {
        let turn_event = UsageEvent::new(
            tenant_id.to_string(),
            tenant_id.to_string(),
            ActionType::AgentTurn,
            1,
        )
        .with_properties(serde_json::json!({
            "model": model,
            "duration_ms": duration_ms,
        }));
        if let Err(e) = billing.report_usage(turn_event).await {
            warn!(error = %e, "metering: report_usage(agent_turn) failed");
        }
        quota.record(tenant_id, &ActionType::AgentTurn, 1).await;

        let total_tokens = input_tokens + output_tokens;
        if total_tokens > 0 {
            let tok_event = UsageEvent::new(
                tenant_id.to_string(),
                tenant_id.to_string(),
                ActionType::Token,
                total_tokens,
            );
            if let Err(e) = billing.report_usage(tok_event).await {
                warn!(error = %e, "metering: report_usage(token) failed");
            }
            quota.record(tenant_id, &ActionType::Token, total_tokens).await;
        }
    }
}
