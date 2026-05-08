//! UI API router — session-authenticated JSON/SSE endpoints.
//! HTML is served by the SvelteKit frontend; this module provides only the backend API.

use crate::state::AppState;
use crate::ui::handlers::{chat, invoice, upload};
use axum::{
    Router,
    routing::post,
};
use std::sync::Arc;

pub fn ui_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/ui/stream", post(chat::ui_stream))
        .route("/ui/upload", post(upload::ui_upload))
        .route("/ui/extract-invoice", post(invoice::ui_extract_invoice))
}
