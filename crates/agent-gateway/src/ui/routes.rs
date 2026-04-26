//! UI router — auth, app shell, chat stream, upload.

use crate::state::AppState;
use crate::ui::handlers::{app, auth, chat, invoice, upload};
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

pub fn ui_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(app::index))
        .route("/login", get(auth::login_get).post(auth::login_post))
        .route("/logout", get(auth::logout))
        .route("/ui/stream", post(chat::ui_stream))
        .route("/ui/upload", post(upload::ui_upload))
        .route("/ui/extract-invoice", post(invoice::ui_extract_invoice))
}
