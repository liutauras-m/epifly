//! File download endpoint.
//! GET /ui/files/download?key={object_key}
//!
//! Streams a file directly from RustFS by its object key.
//! Optionally verifies session + tenant prefix ownership.

use crate::state::AppState;
use crate::ui::session::{COOKIE_NAME, verify as verify_session};
use axum::{
    body::Body,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use axum_extra::extract::CookieJar;
use object_store::{ObjectStore, path::Path as OsPath};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct DownloadQuery {
    pub key: String,
}

pub async fn ui_download(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    Query(q): Query<DownloadQuery>,
) -> Response {
    let store = match state.file_store.as_ref() {
        Some(s) => s,
        None => return err(StatusCode::SERVICE_UNAVAILABLE, "file storage not configured"),
    };

    // If a session is present, verify the object key belongs to that tenant.
    if let Some(session_value) = jar.get(COOKIE_NAME).map(|c| c.value().to_string()) {
        if let Some(u) = verify_session(&session_value) {
            let tenant_id = u.tenant_context().tenant_id;
            let expected_prefix = format!("tenants/{}/", tenant_id.as_str());
            if !q.key.starts_with(&expected_prefix) {
                return err(StatusCode::FORBIDDEN, "object does not belong to your tenant");
            }
        }
    }

    let os_path = OsPath::from(q.key.as_str());
    let result = match store.get(&os_path).await {
        Ok(r) => r,
        Err(e) => return err(StatusCode::NOT_FOUND, &format!("object not found: {e}")),
    };
    let bytes = match result.bytes().await {
        Ok(b) => b,
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, &format!("read error: {e}")),
    };

    let filename = q.key.split('/').next_back().unwrap_or("file");
    Response::builder()
        .status(200)
        .header("content-type", "application/octet-stream")
        .header("content-disposition", format!("attachment; filename=\"{filename}\""))
        .header("content-length", bytes.len().to_string())
        .header("cache-control", "private, no-store")
        .body(Body::from(bytes))
        .expect("response build failed")
}

fn err(code: StatusCode, msg: &str) -> Response {
    (code, msg.to_string()).into_response()
}
