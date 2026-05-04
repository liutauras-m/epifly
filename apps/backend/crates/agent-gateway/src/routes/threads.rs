use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use axum::{
    Extension, Json,
    extract::{Path, State},
    response::IntoResponse,
};
use common::error::HttpError;
use std::sync::Arc;

/// `GET /v1/threads/{id}/messages` — list messages for a thread.
///
/// Returns `{ "data": [...messages...] }` (OpenAI-compatible envelope).
pub async fn get_messages(
    State(state): State<Arc<AppState>>,
    Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
    Path(thread_id): Path<String>,
) -> impl IntoResponse {
    match state
        .thread_store
        .messages(&tenant.tenant_id, &thread_id)
        .await
    {
        Ok(messages) => Ok(Json(serde_json::json!({ "data": messages }))),
        Err(e) => Err(HttpError::agent(format!("thread store error: {e}"))),
    }
}
