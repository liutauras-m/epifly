use crate::state::AppState;
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

mod capabilities;
mod chat;
mod health;

/// Routes that require no auth (health probe, etc.)
pub fn public_router() -> Router<Arc<AppState>> {
    Router::new().route("/health", get(health::health))
}

/// Routes protected by the tenant middleware.
pub fn protected_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/v1/chat/completions", post(chat::completions))
        .route("/v1/capabilities", get(capabilities::list_capabilities))
}
