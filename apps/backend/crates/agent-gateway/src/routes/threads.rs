use crate::agent::StreamState;
use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use agent_core::store::ProjectionStatus;
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    response::IntoResponse,
};
use common::error::HttpError;
use serde::Deserialize;
use std::sync::Arc;

/// `GET /v1/threads/{id}/status` — live stream + projection status for UI indicators.
///
/// Returns `{ running, run_id, projection_status }` where:
/// - `running`: whether an agent turn is currently streaming for this thread
/// - `run_id`: the active run identifier if running, null otherwise
/// - `projection_status`: `"active"` | `"paused"` | `"error"` | `"none"`
pub async fn thread_status(
    State(state): State<Arc<AppState>>,
    Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
    Path(thread_id): Path<String>,
) -> impl IntoResponse {
    let rt = state
        .thread_runtime_registry
        .get(&tenant.tenant_id, &thread_id);

    let (running, run_id) = match rt {
        Some(ref r) => {
            let state_guard = r.stream_state.read();
            match &*state_guard {
                StreamState::Running { run_id } => (true, Some(run_id.clone())),
                _ => (false, None),
            }
        }
        None => (false, None),
    };

    let projection_status = match state
        .thread_projection_store
        .get(&tenant.tenant_id, &thread_id)
        .await
    {
        Ok(Some(proj)) => match proj.status {
            ProjectionStatus::Active => "active",
            ProjectionStatus::Paused => "paused",
            ProjectionStatus::Error => "error",
        },
        Ok(None) => "none",
        Err(e) => return Err(HttpError::agent(format!("projection store error: {e}"))),
    };

    Ok(Json(serde_json::json!({
        "running": running,
        "run_id": run_id,
        "projection_status": projection_status,
    })))
}

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

/// Query parameters for `GET /v1/threads`.
#[derive(Debug, Deserialize, Default)]
pub struct ListQuery {
    /// Maximum number of threads to return. Defaults to `20`, clamped to `100`.
    #[serde(default)]
    pub limit: Option<usize>,
    /// Optional ULID cursor — return threads whose `last_active` is strictly before
    /// the thread identified by `after`. Newest-first ordering.
    #[serde(default)]
    pub after: Option<String>,
}

/// `GET /v1/threads` — list threads for the tenant, newest first (PR 3.A.6).
///
/// Tenant-scoped via `ResolvedTenant`. Pagination via `?limit=` (default 20, max
/// 100) and `?after=<ulid>` for cursor-based scroll. Returns
/// `{ "data": [{ id, title?, last_active, message_count }, ...] }`.
pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(ResolvedTenant(tenant)): Extension<ResolvedTenant>,
    Query(q): Query<ListQuery>,
) -> impl IntoResponse {
    let limit = q.limit.unwrap_or(20).clamp(1, 100);
    let after = q.after.as_deref();
    match state
        .thread_store
        .list(&tenant.tenant_id, limit, after)
        .await
    {
        Ok(threads) => {
            let payload: Vec<_> = threads
                .into_iter()
                .map(|t| {
                    serde_json::json!({
                        "id": t.id.to_string(),
                        "title": t.title,
                        "last_active": t.last_active,
                        "message_count": t.message_count,
                    })
                })
                .collect();
            Ok(Json(serde_json::json!({ "data": payload })))
        }
        Err(e) => Err(HttpError::agent(format!("thread store error: {e}"))),
    }
}
