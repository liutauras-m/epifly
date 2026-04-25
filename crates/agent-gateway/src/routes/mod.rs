use axum::{Router, routing::{get, post}};
use std::sync::Arc;
use crate::state::AppState;

mod chat;
mod health;
mod capabilities;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/health", get(health::health))
        .route("/v1/chat/completions", post(chat::completions))
        .route("/v1/capabilities", get(capabilities::list_capabilities))
}
