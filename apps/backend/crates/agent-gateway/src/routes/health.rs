use crate::state::AppState;
use axum::{Json, extract::State};
use serde_json::{Value, json};
use std::sync::Arc;

#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Service is healthy", body = Value),
    ),
    tag = "health",
)]
pub async fn health(State(state): State<Arc<AppState>>) -> Json<Value> {
    let cap_count = state.registry.lock().unwrap().len();
    let postgres_status = match &state.pool {
        Some(pool) => match sqlx::query("SELECT 1").execute(pool).await {
            Ok(_) => "ok",
            Err(_) => "unreachable",
        },
        None => "disabled (test mode)",
    };
    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "capabilities": cap_count,
        "postgres": postgres_status,
    }))
}
