//! UI file upload — multipart → MinIO, returns token + filename for the composer chip.

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
        None => {
            return err(
                StatusCode::SERVICE_UNAVAILABLE,
                "file storage not configured",
            );
        }
    };

    let tenant = user.tenant_context();

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

    let object_key = format!("{}{}/{}", tenant.storage_prefix(), Uuid::new_v4(), filename);
    let os_path = OsPath::from(object_key.as_str());

    if let Err(e) = store.put(&os_path, data.into()).await {
        warn!(error = %e, "ui upload write failed");
        return err(StatusCode::INTERNAL_SERVER_ERROR, &format!("storage: {e}"));
    }

    let token = Uuid::new_v4().to_string();
    {
        let mut tokens = state.presigned_tokens.lock().unwrap();
        tokens.insert(
            token.clone(),
            (
                object_key,
                std::time::Instant::now(),
                std::time::Duration::from_secs(3600),
                tenant.tenant_id.to_string(),
            ),
        );
    }

    let payload: Value = json!({
        "id": token,
        "filename": filename,
        "size": size,
        "content_type": content_type,
        "download_url": format!("/ui/files/{token}"),
    });
    (StatusCode::OK, Json(payload)).into_response()
}

fn err(code: StatusCode, msg: &str) -> Response {
    (code, Json(json!({ "error": msg }))).into_response()
}
