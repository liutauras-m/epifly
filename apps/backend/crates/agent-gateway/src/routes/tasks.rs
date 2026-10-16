//! `/v1/tasks` — query background task status and subscribe to SSE streams.
//!
//! Every handler is scoped to the caller's tenant: another tenant's task id is
//! answered exactly like an unknown id (404).

use crate::auth::ResolvedTenant;
use crate::state::AppState;
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Sse, sse::Event},
};
use futures::StreamExt;
use jobs::TaskStatus;
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;
use tokio_stream::wrappers::BroadcastStream;
use uuid::Uuid;

// ── List tasks ────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ListQuery {
    #[serde(default = "default_limit")]
    limit: usize,
}
fn default_limit() -> usize {
    50
}

pub async fn list_tasks(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    axum::extract::Query(q): axum::extract::Query<ListQuery>,
) -> Json<Vec<TaskStatus>> {
    Json(
        state
            .job_executor
            .list_tasks_for_tenant(tenant.0.tenant_id.as_str(), q.limit.min(200))
            .await,
    )
}

// ── Get single task ───────────────────────────────────────────────────────────

pub async fn get_task(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match state
        .job_executor
        .get_status_for_tenant(id, tenant.0.tenant_id.as_str())
        .await
    {
        Some(s) => (StatusCode::OK, Json(s)).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

// ── SSE stream ────────────────────────────────────────────────────────────────

pub async fn task_sse(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let tenant_id = tenant.0.tenant_id.as_str();
    let Some(status) = state
        .job_executor
        .get_status_for_tenant(id, tenant_id)
        .await
    else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match state.job_executor.subscribe_for_tenant(id, tenant_id).await {
        None => {
            // Task already complete — send a single terminal event.
            let data = serde_json::to_string(&status).unwrap_or_default();
            let stream = futures::stream::once(async move {
                Ok::<Event, std::convert::Infallible>(
                    Event::default().event("task_update").data(data),
                )
            });
            Sse::new(stream)
                .keep_alive(axum::response::sse::KeepAlive::new().interval(Duration::from_secs(15)))
                .into_response()
        }
        Some(rx) => {
            let stream = BroadcastStream::new(rx).filter_map(|ev| async move {
                match ev {
                    Ok(ev) => {
                        let data = serde_json::to_string(&serde_json::json!({
                            "task_id": ev.task_id,
                            "state": ev.state,
                            "result": ev.result,
                            "error": ev.error,
                        }))
                        .unwrap_or_default();
                        Some(Ok::<Event, std::convert::Infallible>(
                            Event::default().event("task_update").data(data),
                        ))
                    }
                    Err(_) => None,
                }
            });
            Sse::new(stream)
                .keep_alive(axum::response::sse::KeepAlive::new().interval(Duration::from_secs(15)))
                .into_response()
        }
    }
}
