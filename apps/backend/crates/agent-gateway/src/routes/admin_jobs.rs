//! Super-admin REST API for managing scheduled and background jobs.
//!
//! All routes require `Authorization: Bearer <jwt>` with `role = "super_admin"`.

use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use jobs::JobSummary;
use std::sync::Arc;

/// `GET /admin/jobs` — list all registered jobs.
pub async fn list_jobs(
    State(state): State<Arc<AppState>>,
    Extension(_tenant): Extension<ResolvedTenant>,
) -> Json<Vec<JobSummary>> {
    Json(state.job_admin.list_jobs())
}

/// `GET /admin/jobs/{name}` — get a single job summary.
pub async fn get_job(
    State(state): State<Arc<AppState>>,
    Extension(_tenant): Extension<ResolvedTenant>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.job_admin.get_job(&name) {
        Some(j) => (StatusCode::OK, Json(j)).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// `POST /admin/jobs/{name}/run` — enqueue a background job immediately.
pub async fn run_now(
    State(state): State<Arc<AppState>>,
    Extension(_tenant): Extension<ResolvedTenant>,
    Path(name): Path<String>,
    body: Option<Json<serde_json::Value>>,
) -> impl IntoResponse {
    let input = body.map(|b| b.0).unwrap_or(serde_json::Value::Null);
    match state.job_admin.run_now(&name, input).await {
        Ok(task_id) => (
            StatusCode::ACCEPTED,
            Json(serde_json::json!({ "task_id": task_id, "status": "queued" })),
        )
            .into_response(),
        Err(e) => (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()).into_response(),
    }
}

/// `GET /admin/tasks` — list recent task statuses (super-admin only).
pub async fn list_tasks(
    State(state): State<Arc<AppState>>,
    Extension(_tenant): Extension<ResolvedTenant>,
) -> impl IntoResponse {
    Json(state.job_admin.list_tasks(200).await)
}
