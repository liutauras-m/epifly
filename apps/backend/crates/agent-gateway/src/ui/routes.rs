//! UI router — session-authenticated endpoints for streaming, file upload, and invoice extraction.
//! HTML rendering is handled by the SvelteKit frontend (apps/web).

use crate::state::AppState;
use crate::ui::handlers::{chat, files, invoice, upload};
use axum::{
    Router,
    routing::{get, post},
};
use std::sync::Arc;

pub fn ui_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/ui/stream", post(chat::ui_stream))
        .route("/ui/upload", post(upload::ui_upload))
        .route("/ui/files/{token}", get(files::ui_download))
        .route("/ui/extract-invoice", post(invoice::ui_extract_invoice))
}
