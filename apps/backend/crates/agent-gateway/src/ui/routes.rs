//! UI router — streaming chat, upload and download for the web app.
//! Mounted behind the same bearer authentication as `/v1/*`.

use crate::state::AppState;
use crate::ui::handlers::{chat, files, upload};
use axum::{
    Router,
    routing::{get, post},
};
use std::sync::Arc;

pub fn ui_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/ui/stream", post(chat::ui_stream))
        .route("/ui/upload", post(upload::ui_upload))
        .route("/ui/files/download", get(files::ui_download))
}
