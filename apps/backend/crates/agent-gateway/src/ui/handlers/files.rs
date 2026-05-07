//! File download endpoint.
//! GET /ui/files/{token}
//!
//! Two access modes:
//! - Browser (session cookie present): validates session + tenant ownership.
//! - Server-to-server (no session): validates token existence + TTL only.
//!   The token itself is the credential — used by the agent's `resolve_image_path`
//!   when it fetches an image URL to pass to an LLM tool.

use crate::state::AppState;
use crate::ui::session::{COOKIE_NAME, verify as verify_session};
use axum::{
    body::Body,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use axum_extra::extract::CookieJar;
use object_store::{ObjectStore, path::Path as OsPath};
use std::sync::Arc;

pub async fn ui_download(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    Path(token): Path<String>,
) -> Response {
    let store = match state.file_store.as_ref() {
        Some(s) => s,
        None => {
            return err(
                StatusCode::SERVICE_UNAVAILABLE,
                "file storage not configured",
            );
        }
    };

    // Optionally resolve the session user for tenant-ownership checks.
    let session_user = jar.get(COOKIE_NAME).and_then(|c| verify_session(c.value()));

    let object_key = {
        let tokens = state.presigned_tokens.lock().unwrap();
        let (key, created, ttl, stored_tid) = match tokens.get(&token) {
            Some(t) => t,
            None => return err(StatusCode::NOT_FOUND, "download token not found"),
        };
        if created.elapsed() > *ttl {
            return err(StatusCode::GONE, "download token expired");
        }
        // If a session is present, enforce tenant ownership.
        if let Some(ref u) = session_user {
            let tid = u.tenant_context().tenant_id;
            if stored_tid != tid.as_str() {
                return err(
                    StatusCode::FORBIDDEN,
                    "token does not belong to your tenant",
                );
            }
        }
        key.clone()
    };

    let os_path = OsPath::from(object_key.as_str());
    let result = match store.get(&os_path).await {
        Ok(r) => r,
        Err(e) => return err(StatusCode::NOT_FOUND, &format!("object not found: {e}")),
    };
    let bytes = match result.bytes().await {
        Ok(b) => b,
        Err(e) => {
            return err(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("read error: {e}"),
            );
        }
    };

    Response::builder()
        .status(200)
        .header("content-type", "application/octet-stream")
        .header("content-length", bytes.len().to_string())
        .body(Body::from(bytes))
        .expect("response build failed")
}

fn err(code: StatusCode, msg: &str) -> Response {
    (code, msg.to_string()).into_response()
}
