//! Direct invoice extraction — bypasses the agent loop entirely.
//! POST /ui/extract-invoice: object_key → RustFS bytes → capability registry → InvoiceData JSON.
//!
//! Delegates extraction to the `invoice-processing` capability via `ToolExecutor` so that
//! all model calls go through `LlmRegistry` — no direct Anthropic client construction here.

use crate::state::AppState;
use crate::ui::session::SessionUser;
use agent_core::capabilities::executor::ToolExecutor;
use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use object_store::{ObjectStore, path::Path as OsPath};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{info, instrument};

#[derive(Deserialize)]
pub struct ExtractRequest {
    /// Object key in RustFS (replaces the old UUID token).
    pub object_key: String,
}

#[instrument(skip(state, user, body), fields(key = %body.object_key))]
pub async fn ui_extract_invoice(
    State(state): State<Arc<AppState>>,
    user: SessionUser,
    Json(body): Json<ExtractRequest>,
) -> Response {
    let store = match state.file_store.as_ref() {
        Some(s) => s,
        None => return err(StatusCode::SERVICE_UNAVAILABLE, "file storage not configured"),
    };

    // Verify tenant ownership via TenantStorage (layout-aware).
    let tenant = user.tenant_context();
    let owned = if let Some(factory) = state.tenant_storage.as_ref() {
        match factory.for_tenant(tenant.tenant_id.as_str()).await {
            Ok(storage) => storage.owns_object_key(&body.object_key),
            Err(_) => false,
        }
    } else {
        false
    };
    if !owned {
        return err(StatusCode::FORBIDDEN, "object does not belong to your tenant");
    }

    info!(key = %body.object_key, "downloading from object store for invoice extraction");

    let os_path = OsPath::from(body.object_key.as_str());
    let get_result = match store.get(&os_path).await {
        Ok(r) => r,
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, &format!("storage get: {e}")),
    };
    let object_bytes = match get_result.bytes().await {
        Ok(b) => b,
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, &format!("storage read: {e}")),
    };

    // Write bytes to a temp file so the chain executor can read it via image_path.
    let tmp_path = std::env::temp_dir().join(format!("conusai-invoice-{}.bin", uuid::Uuid::new_v4()));
    if let Err(e) = std::fs::write(&tmp_path, &object_bytes) {
        return err(StatusCode::INTERNAL_SERVER_ERROR, &format!("temp write: {e}"));
    }

    info!(bytes = object_bytes.len(), "invoking invoice-processing capability");

    let input = json!({ "image_path": tmp_path.to_string_lossy() });
    let registry = state.registry.lock().unwrap();
    let result: Result<Value, _> =
        ToolExecutor::invoke(&registry, "invoice-processing", "extract_invoice", &input, Some(&tenant))
            .await;

    // Clean up temp file regardless of outcome.
    let _ = std::fs::remove_file(&tmp_path);

    match result {
        Ok(data) => (StatusCode::OK, Json(data)).into_response(),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, &format!("extraction failed: {e}")),
    }
}

fn err(code: StatusCode, msg: &str) -> Response {
    (code, Json(json!({ "error": msg }))).into_response()
}
