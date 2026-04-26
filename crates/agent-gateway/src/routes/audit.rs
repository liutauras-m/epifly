use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use axum::{
    Extension, Json,
    extract::{Query, State},
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct ListQuery {
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    50
}

pub async fn list_audit(
    State(state): State<Arc<AppState>>,
    Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
    Query(q): Query<ListQuery>,
) -> Json<serde_json::Value> {
    let limit = q.limit.min(500);
    match state.audit_store.list(&tenant.tenant_id, limit).await {
        Ok(events) => Json(serde_json::json!({ "events": events, "count": events.len() })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string(), "events": [] })),
    }
}
