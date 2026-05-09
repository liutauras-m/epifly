//! UI router — auth, app shell, chat stream, upload.

use crate::mw::admin::require_super_admin_session;
use crate::state::AppState;
use crate::ui::handlers::{app, auth, chat, files, invoice, super_admin, upload};
use axum::{
    Router,
    middleware,
    routing::{get, post},
};
use std::sync::Arc;

pub fn ui_router() -> Router<Arc<AppState>> {
    let admin_routes = Router::new()
        .route("/super-admin", get(super_admin::index))
        .route("/super-admin/new", get(super_admin::new_form).post(super_admin::create))
        .route("/super-admin/reload-all", post(super_admin::reload_all_caps))
        .route("/super-admin/{name}", get(super_admin::detail).post(super_admin::update))
        .route("/super-admin/{name}/toggle", post(super_admin::toggle_enabled))
        .route("/super-admin/{name}/delete", post(super_admin::delete_cap))
        .route("/super-admin/{name}/reload", post(super_admin::reload_cap))
        .layer(middleware::from_fn(require_super_admin_session));

    Router::new()
        .route("/", get(app::index))
        .route("/login", get(auth::login_get).post(auth::login_post))
        .route("/logout", get(auth::logout))
        .route("/ui/stream", post(chat::ui_stream))
        .route("/ui/upload", post(upload::ui_upload))
        .route("/ui/files/{token}", get(files::ui_download))
        .route("/ui/extract-invoice", post(invoice::ui_extract_invoice))
        .merge(admin_routes)
}
