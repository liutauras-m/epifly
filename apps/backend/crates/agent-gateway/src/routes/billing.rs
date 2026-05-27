use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use billing_core::types::{Invoice, Subscription};
use common::error::HttpError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ── Request / response types ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateSubscriptionRequest {
    pub plan_key: String,
    pub return_url: String,
}

#[derive(Debug, Deserialize)]
pub struct PortalRequest {
    pub return_url: String,
}

/// Query string for `/v1/billing/usage`. Date range is accepted to keep the
/// public API stable; values are ignored until TimescaleDB rollups land.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct UsageQuery {
    pub from: Option<String>,
    pub to: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PortalResponse {
    pub url: String,
}

// ── GET /v1/billing/plans ─────────────────────────────────────────────────────

pub async fn list_plans(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(state.plan_catalog.list().to_vec())
}

// ── GET /v1/billing/subscription ─────────────────────────────────────────────

pub async fn get_subscription(
    State(state): State<Arc<AppState>>,
    tenant: Option<axum::Extension<ResolvedTenant>>,
) -> Response {
    let Some(axum::Extension(ResolvedTenant(ctx))) = tenant else {
        return HttpError::auth("authentication required").into_response();
    };

    let Some(billing) = &state.billing else {
        return (
            StatusCode::NOT_IMPLEMENTED,
            Json(serde_json::json!({
                "error": "billing not configured"
            })),
        )
            .into_response();
    };

    match billing.get_subscription(&ctx.tenant_id).await {
        Ok(sub) => Json(sub).into_response(),
        Err(billing_core::BillingError::SubscriptionNotFound(_)) => {
            // Return a default Free subscription for tenants without one.
            Json(Subscription {
                tenant_id: ctx.tenant_id.to_string(),
                lago_customer_id: ctx.tenant_id.to_string(),
                lago_subscription_id: None,
                plan_key: "free".into(),
                status: billing_core::types::SubscriptionStatus::Active,
                current_period_start: None,
                current_period_end: None,
            })
            .into_response()
        }
        Err(e) => {
            tracing::warn!(error = %e, "get_subscription failed");
            HttpError::internal("billing service error", None).into_response()
        }
    }
}

// ── POST /v1/billing/subscriptions ───────────────────────────────────────────

pub async fn create_subscription(
    State(state): State<Arc<AppState>>,
    tenant: Option<axum::Extension<ResolvedTenant>>,
    Json(req): Json<CreateSubscriptionRequest>,
) -> Response {
    let Some(axum::Extension(ResolvedTenant(ctx))) = tenant else {
        return HttpError::auth("authentication required").into_response();
    };

    let Some(billing) = &state.billing else {
        return (StatusCode::NOT_IMPLEMENTED, "billing not configured").into_response();
    };

    match billing
        .create_or_update_subscription(&ctx.tenant_id, &req.plan_key, &req.return_url)
        .await
    {
        Ok(session) => Json(session).into_response(),
        Err(e) => {
            tracing::warn!(error = %e, plan_key = %req.plan_key, "create_subscription failed");
            HttpError::internal("billing service error", None).into_response()
        }
    }
}

// ── POST /v1/billing/portal ───────────────────────────────────────────────────

pub async fn billing_portal(
    State(state): State<Arc<AppState>>,
    tenant: Option<axum::Extension<ResolvedTenant>>,
    Json(req): Json<PortalRequest>,
) -> Response {
    let Some(axum::Extension(ResolvedTenant(ctx))) = tenant else {
        return HttpError::auth("authentication required").into_response();
    };

    let Some(billing) = &state.billing else {
        return (StatusCode::NOT_IMPLEMENTED, "billing not configured").into_response();
    };

    match billing.portal_url(&ctx.tenant_id, &req.return_url).await {
        Ok(url) => Json(PortalResponse { url }).into_response(),
        Err(e) => {
            tracing::warn!(error = %e, "billing_portal failed");
            HttpError::internal("billing service error", None).into_response()
        }
    }
}

// ── DELETE /v1/billing/subscription ──────────────────────────────────────────

pub async fn cancel_subscription(
    State(state): State<Arc<AppState>>,
    tenant: Option<axum::Extension<ResolvedTenant>>,
) -> Response {
    let Some(axum::Extension(ResolvedTenant(ctx))) = tenant else {
        return HttpError::auth("authentication required").into_response();
    };

    let Some(billing) = &state.billing else {
        return (StatusCode::NOT_IMPLEMENTED, "billing not configured").into_response();
    };

    match billing.cancel_subscription(&ctx.tenant_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            tracing::warn!(error = %e, "cancel_subscription failed");
            HttpError::internal("billing service error", None).into_response()
        }
    }
}

// ── GET /v1/billing/invoices ──────────────────────────────────────────────────

pub async fn list_invoices(
    State(state): State<Arc<AppState>>,
    tenant: Option<axum::Extension<ResolvedTenant>>,
) -> Response {
    let Some(axum::Extension(ResolvedTenant(ctx))) = tenant else {
        return HttpError::auth("authentication required").into_response();
    };

    let Some(billing) = &state.billing else {
        return Json(Vec::<Invoice>::new()).into_response();
    };

    match billing.list_invoices(&ctx.tenant_id).await {
        Ok(invoices) => Json(invoices).into_response(),
        Err(e) => {
            tracing::warn!(error = %e, "list_invoices failed");
            Json(Vec::<Invoice>::new()).into_response()
        }
    }
}

// ── GET /v1/billing/usage ─────────────────────────────────────────────────────

pub async fn get_usage(
    State(_state): State<Arc<AppState>>,
    tenant: Option<axum::Extension<ResolvedTenant>>,
    Query(_q): Query<UsageQuery>,
) -> Response {
    let Some(axum::Extension(ResolvedTenant(_ctx))) = tenant else {
        return HttpError::auth("authentication required").into_response();
    };

    // Placeholder: return empty usage until TimescaleDB rollups are wired in.
    Json(serde_json::json!({
        "agent_turns": 0,
        "tokens": 0,
        "storage_gb": 0.0
    }))
    .into_response()
}

// ── Admin billing routes ───────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AddCreditsRequest {
    pub tenant_id: String,
    pub amount_cents: i64,
    pub description: Option<String>,
}

/// POST /admin/billing/credits — add wallet credits to a tenant's Lago account.
pub async fn admin_add_credits(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AddCreditsRequest>,
) -> Response {
    let Some(billing) = &state.billing else {
        return (StatusCode::NOT_IMPLEMENTED, "billing not configured").into_response();
    };

    match billing
        .add_credits(&req.tenant_id, req.amount_cents, req.description.as_deref())
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            tracing::warn!(error = %e, "admin_add_credits failed");
            HttpError::internal("billing service error", None).into_response()
        }
    }
}

/// POST /admin/billing/cancel/:tenant_id — manually cancel a tenant's subscription.
pub async fn admin_cancel_subscription(
    State(state): State<Arc<AppState>>,
    Path(tenant_id): Path<String>,
) -> Response {
    use common::types::TenantId;
    let Some(billing) = &state.billing else {
        return (StatusCode::NOT_IMPLEMENTED, "billing not configured").into_response();
    };

    let tid = TenantId::new(tenant_id.as_str());
    match billing.cancel_subscription(&tid).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            tracing::warn!(error = %e, %tenant_id, "admin_cancel_subscription failed");
            HttpError::internal("billing service error", None).into_response()
        }
    }
}

/// GET /admin/billing/dashboard — aggregated usage + revenue summary across all tenants.
pub async fn admin_billing_dashboard(State(state): State<Arc<AppState>>) -> Response {
    let Some(billing) = &state.billing else {
        return Json(serde_json::json!({ "configured": false })).into_response();
    };

    match billing.analytics_summary().await {
        Ok(summary) => Json(summary).into_response(),
        Err(e) => {
            tracing::warn!(error = %e, "admin_billing_dashboard failed");
            HttpError::internal("billing service error", None).into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mw::tenant::ResolvedTenant;
    use agent_core::{PlanTier, TenantContext};
    use axum::{Extension, body::to_bytes};

    fn test_tenant() -> Extension<ResolvedTenant> {
        Extension(ResolvedTenant(TenantContext::new(
            "tenant-disabled-billing",
            None::<&str>,
            PlanTier::Free,
            "/tmp/conusai-tests",
        )))
    }

    #[tokio::test]
    async fn get_subscription_reports_billing_not_configured_when_provider_missing() {
        let state = Arc::new(AppState::with_in_memory_stores().expect("state"));

        let resp = get_subscription(State(state), Some(test_tenant())).await;
        assert_eq!(resp.status(), StatusCode::NOT_IMPLEMENTED);

        let body = to_bytes(resp.into_body(), usize::MAX).await.expect("body");
        let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(
            json,
            serde_json::json!({ "error": "billing not configured" })
        );
    }

    #[tokio::test]
    async fn admin_billing_dashboard_marks_billing_unconfigured_when_provider_missing() {
        let state = Arc::new(AppState::with_in_memory_stores().expect("state"));

        let resp = admin_billing_dashboard(State(state)).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let body = to_bytes(resp.into_body(), usize::MAX).await.expect("body");
        let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert_eq!(json, serde_json::json!({ "configured": false }));
    }
}
