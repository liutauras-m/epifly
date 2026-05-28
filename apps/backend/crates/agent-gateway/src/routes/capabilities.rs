use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use agent_core::PlanLimits;
use axum::{Extension, Json, extract::State, response::IntoResponse};
use common::error::HttpError;
use serde_json::{Value, json};
use std::sync::Arc;

#[utoipa::path(
    get,
    path = "/v1/capabilities",
    responses(
        (status = 200, description = "List of registered capabilities", body = Value),
        (status = 429, description = "Rate limit exceeded"),
    ),
    security(("bearer_auth" = [])),
    tag = "capabilities",
)]
pub async fn list_capabilities(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    Extension(limits): Extension<PlanLimits>,
) -> impl IntoResponse {
    if !state
        .rate_limiter
        .check(&tenant.0.tenant_id, limits.rate_limit_rpm)
    {
        return HttpError::rate_limit(None).into_response();
    }
    let registry = state.registry.read();
    let model = std::env::var("ANTHROPIC_MODEL").unwrap_or_else(|_| "claude-opus-4-7".into());
    let plan_max_turns = limits.max_turns;
    let caps: Vec<Value> = registry
        .enabled_for_tenant(&tenant.0.tenant_id)
        .map(|card| {
            let supported_tools: Vec<_> =
                card.manifest.tools.iter().map(|t| t.name.clone()).collect();
            json!({
                "name": card.manifest.name,
                "namespace": card.manifest.namespace.clone().unwrap_or_default(),
                "category": card.manifest.category.clone().unwrap_or_default(),
                "version": card.manifest.version,
                "description": card.manifest.description,
                "kind": format!("{:?}", card.manifest.kind),
                "tags": card.manifest.tags,
                "tools": card.manifest.tools.iter().map(|t| json!({
                    "name": t.name,
                    "description": t.description,
                })).collect::<Vec<_>>(),
                "models": [&model],
                "max_turns_limit": plan_max_turns,
                "supported_tools": supported_tools,
            })
        })
        .collect();
    Json(json!({
        "tenant_id": tenant.0.tenant_id,
        "plan": tenant.0.plan.to_string(),
        "model": model,
        "max_turns_limit": plan_max_turns,
        "capabilities": caps,
    }))
    .into_response()
}
