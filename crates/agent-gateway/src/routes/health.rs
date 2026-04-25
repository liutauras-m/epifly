use crate::state::AppState;
use axum::{extract::State, Json};
use serde_json::{json, Value};
use std::sync::Arc;

pub async fn health(State(state): State<Arc<AppState>>) -> Json<Value> {
    let cap_count = state.registry.lock().unwrap().len();
    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "capabilities": cap_count,
    }))
}
