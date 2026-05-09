use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use axum::{
    Extension, Json,
    extract::{Query, State},
    response::IntoResponse,
};
use common::error::HttpError;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize, utoipa::IntoParams)]
pub struct ListQuery {
    #[serde(default = "default_limit")]
    limit: usize,
    /// Opaque cursor (event `id`) — only events older than this are returned.
    after: Option<String>,
}

fn default_limit() -> usize {
    50
}

#[utoipa::path(
    get,
    path = "/v1/audit",
    params(ListQuery),
    responses(
        (status = 200, description = "Audit events for the calling tenant", body = serde_json::Value),
    ),
    security(("bearer_auth" = [])),
    tag = "audit",
)]
pub async fn list_audit(
    State(state): State<Arc<AppState>>,
    Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
    Query(q): Query<ListQuery>,
) -> impl IntoResponse {
    let limit = q.limit.min(500);
    match state.audit_store.list(&tenant.tenant_id, limit, q.after.as_deref()).await {
        Ok(events) => {
            let next_cursor = events.last().map(|e| e.id.clone());
            Json(serde_json::json!({
                "events": events,
                "count": events.len(),
                "has_more": events.len() == limit,
                "next_cursor": next_cursor,
            }))
            .into_response()
        }
        Err(e) => HttpError::internal(e.to_string(), None).into_response(),
    }
}
