use crate::state::AppState;
use axum::{Json, extract::State};
use serde_json::{Value, json};
use std::sync::Arc;

pub async fn health(State(state): State<Arc<AppState>>) -> Json<Value> {
    let cap_count = state.registry.lock().unwrap().len();
    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "capabilities": cap_count,
    }))
}
