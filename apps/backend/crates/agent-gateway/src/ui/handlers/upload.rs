//! UI file upload — multipart → RustFS, returns object key for the composer chip.
//! No in-memory token map — the object key is the durable reference.

use crate::state::AppState;
use crate::ui::session::SessionUser;
use axum::{
    Json,
    extract::{Multipart, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use object_store::{ObjectStore, path::Path as OsPath};
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::warn;
use uuid::Uuid;

pub async fn ui_upload(
    State(state): State<Arc<AppState>>,
    user: SessionUser,
    mut multipart: Multipart,
) -> Response {
    let store = match state.file_store.as_ref() {
        Some(s) => s,
        None => return err(StatusCode::SERVICE_UNAVAILABLE, "file storage not configured"),
    };

    let tenant = user.tenant_context();

    let storage_factory = match state.tenant_storage.as_ref() {
        Some(f) => f,
        None => return err(StatusCode::SERVICE_UNAVAILABLE, "storage not configured"),
    };
    let storage = match storage_factory.for_tenant(tenant.tenant_id.as_str()).await {
        Ok(s) => s,
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, &format!("storage: {e}")),
    };

    let field = match multipart.next_field().await {
        Ok(Some(f)) => f,
        Ok(None) => return err(StatusCode::BAD_REQUEST, "no file in upload"),
        Err(e) => return err(StatusCode::BAD_REQUEST, &format!("multipart: {e}")),
    };

    let filename = field
        .file_name()
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{}.bin", Uuid::new_v4()));
    let content_type = field
        .content_type()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "application/octet-stream".into());

    let data = match field.bytes().await {
        Ok(b) => b,
        Err(e) => return err(StatusCode::BAD_REQUEST, &format!("read: {e}")),
    };
    let size = data.len();

    let object_key = storage.attachment_s3_key(&Uuid::new_v4().to_string(), &filename);
    let os_path = OsPath::from(object_key.as_str());

    if let Err(e) = store.put(&os_path, data.into()).await {
        warn!(error = %e, "ui upload write failed");
        return err(StatusCode::INTERNAL_SERVER_ERROR, &format!("storage: {e}"));
    }

    let payload: Value = json!({
        "id": object_key,
        "filename": filename,
        "size": size,
        "content_type": content_type,
        "download_url": format!("/ui/files/download?key={}", urlencoding::encode(&object_key)),
    });
    (StatusCode::OK, Json(payload)).into_response()
}

fn err(code: StatusCode, msg: &str) -> Response {
    (code, Json(json!({ "error": msg }))).into_response()
}

fn urlencoding_encode(s: &str) -> String {
    s.chars()
        .flat_map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '~' || c == '/' {
                vec![c]
            } else {
                format!("%{:02X}", c as u32).chars().collect()
            }
        })
        .collect()
}

mod urlencoding {
    pub fn encode(s: &str) -> String {
        super::urlencoding_encode(s)
    }
}
