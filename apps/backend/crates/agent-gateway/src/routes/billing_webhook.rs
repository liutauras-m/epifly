/// POST /v1/billing/webhooks — public route (mounted in public_router).
///
/// Verifies Lago webhook HMAC signature then dispatches on `event_type`:
/// - subscription.started|updated|terminated → update_plan_claim
/// - invoice.payment_succeeded / invoice.payment_failed → audit log
/// - customer.usage.threshold_reached → SSE quota warning
///
/// Idempotency: processed `webhook_id` values are cached for 90 days
/// in a process-local Moka cache to reject Lago replays.
use crate::state::AppState;
use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use moka::future::Cache;
use serde::Deserialize;
use std::sync::{Arc, OnceLock};
use tracing::{info, warn};

static WEBHOOK_DEDUP: OnceLock<Cache<String, ()>> = OnceLock::new();

fn dedup_cache() -> &'static Cache<String, ()> {
    WEBHOOK_DEDUP.get_or_init(|| {
        Cache::builder()
            .max_capacity(500_000)
            .time_to_live(std::time::Duration::from_secs(90 * 24 * 3600))
            .build()
    })
}

#[derive(Debug, Deserialize)]
struct WebhookPayload {
    /// Lago-assigned unique ID for idempotency deduplication.
    webhook_id: Option<String>,
    #[serde(rename = "webhook_type")]
    event_type: Option<String>,
    object: Option<serde_json::Value>,
}

pub async fn handle_webhook(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let signature = headers
        .get("X-Lago-Signature")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    // Verify signature if billing is configured.
    if let Some(billing) = &state.billing {
        if let Err(e) = billing.verify_webhook(&body, signature) {
            warn!(error = %e, "webhook signature verification failed");
            return StatusCode::UNAUTHORIZED.into_response();
        }
    }

    let payload: WebhookPayload = match serde_json::from_slice(&body) {
        Ok(p) => p,
        Err(e) => {
            warn!(error = %e, "webhook payload parse error");
            return StatusCode::BAD_REQUEST.into_response();
        }
    };

    // Idempotency check — reject replayed events.
    if let Some(ref webhook_id) = payload.webhook_id {
        let cache = dedup_cache();
        if cache.get(webhook_id).await.is_some() {
            info!(webhook_id, "duplicate webhook received, ignoring");
            return StatusCode::OK.into_response();
        }
        cache.insert(webhook_id.clone(), ()).await;
    }

    let event_type = payload.event_type.as_deref().unwrap_or("");

    match event_type {
        "subscription.started" | "subscription.updated" | "subscription.terminated" => {
            handle_subscription_event(state, event_type, payload.object).await;
        }
        "invoice.payment_succeeded" => {
            info!("invoice payment succeeded");
        }
        "invoice.payment_failed" => {
            warn!("invoice payment failed — dunning initiated");
        }
        "customer.usage.threshold_reached" => {
            if let Some(obj) = &payload.object {
                if let Some(customer_id) = obj.get("external_id").and_then(|v| v.as_str()) {
                    // Push SSE quota warning to the affected tenant.
                    state.realtime_service.broadcast_quota_warning(customer_id).await;
                }
            }
        }
        other => {
            info!(event_type = other, "unhandled Lago webhook event");
        }
    }

    StatusCode::OK.into_response()
}

async fn handle_subscription_event(
    state: Arc<AppState>,
    event_type: &str,
    obj: Option<serde_json::Value>,
) {
    let Some(obj) = obj else { return };

    let customer_id = obj
        .get("external_customer_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let plan_code = obj
        .get("plan_code")
        .and_then(|v| v.as_str())
        .unwrap_or("free");
    let status_str = obj
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("active");

    let plan_tier = match plan_code {
        "pro" => agent_core::PlanTier::Pro,
        "team" | "enterprise" => agent_core::PlanTier::Enterprise,
        _ => agent_core::PlanTier::Free,
    };
    let sub_status = match status_str {
        "active" => agent_core::SubscriptionStatus::Active,
        "pending" => agent_core::SubscriptionStatus::Trialing,
        "terminated" => agent_core::SubscriptionStatus::Canceled,
        _ => agent_core::SubscriptionStatus::Active,
    };

    if !customer_id.is_empty() {
        let tenant_id = common::types::TenantId::from(customer_id.to_string());
        if let Err(e) = state
            .identity
            .update_plan_claim(&tenant_id, plan_tier, sub_status)
            .await
        {
            warn!(
                error = %e,
                event_type,
                customer_id,
                "update_plan_claim failed after subscription webhook"
            );
        } else {
            info!(event_type, customer_id, plan_code, "plan claim updated");
            // Notify live UIs via SSE.
            state
                .realtime_service
                .broadcast_subscription_updated(customer_id)
                .await;
        }
    }
}
