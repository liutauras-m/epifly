//! Direct invoice extraction — bypasses the agent loop entirely.
//! POST /ui/extract-invoice: object_key → RustFS bytes → InvoicePipeline → InvoiceData JSON.

use crate::state::AppState;
use crate::ui::session::SessionUser;
use agent_core::chains::invoice::InvoicePipeline;
use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use object_store::{ObjectStore, path::Path as OsPath};
use serde::Deserialize;
use serde_json::json;
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

    // Verify tenant ownership: key must be under tenants/{tenant_id}/
    let tenant = user.tenant_context();
    let expected_prefix = format!("tenants/{}/", tenant.tenant_id.as_str());
    if !body.object_key.starts_with(&expected_prefix) {
        return err(StatusCode::FORBIDDEN, "object does not belong to your tenant");
    }

    info!(key = %body.object_key, "downloading from object store for invoice extraction");

    let os_path = OsPath::from(body.object_key.as_str());
    let get_result = match store.get(&os_path).await {
        Ok(r) => r,
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, &format!("storage get: {e}")),
    };
    let bytes = match get_result.bytes().await {
        Ok(b) => b,
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, &format!("storage read: {e}")),
    };

    info!(bytes = bytes.len(), "running InvoicePipeline::extract_from_bytes");

    let chain = InvoicePipeline::new();
    match chain.extract_from_bytes(&bytes).await {
        Ok(data) => (StatusCode::OK, Json(data)).into_response(),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, &format!("extraction failed: {e}")),
    }
}

fn err(code: StatusCode, msg: &str) -> Response {
    (code, Json(json!({ "error": msg }))).into_response()
}
