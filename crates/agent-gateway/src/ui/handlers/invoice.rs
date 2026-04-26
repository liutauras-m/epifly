//! Direct invoice extraction — bypasses the agent loop entirely.
//! POST /ui/extract-invoice: token → MinIO bytes → InvoicePipeline → InvoiceData JSON.

use crate::state::AppState;
use crate::ui::session::SessionUser;
use agent_core::pipelines::invoice::InvoicePipeline;
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
    pub token: String,
}

#[instrument(skip(state, _user, body), fields(token = %body.token))]
pub async fn ui_extract_invoice(
    State(state): State<Arc<AppState>>,
    _user: SessionUser,
    Json(body): Json<ExtractRequest>,
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

    // Resolve token → object key (same map used by presigned download)
    let object_key = {
        let tokens = state.presigned_tokens.lock().unwrap();
        match tokens.get(&body.token) {
            Some((key, created, ttl)) => {
                if created.elapsed() > *ttl {
                    return err(StatusCode::GONE, "upload token expired");
                }
                key.clone()
            }
            None => {
                return err(
                    StatusCode::NOT_FOUND,
                    "token not found — upload the file first",
                );
            }
        }
    };

    info!(key = %object_key, "downloading from object store for invoice extraction");

    // Download bytes from MinIO
    let os_path = OsPath::from(object_key.as_str());
    let get_result = match store.get(&os_path).await {
        Ok(r) => r,
        Err(e) => {
            return err(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("storage get: {e}"),
            );
        }
    };
    let bytes = match get_result.bytes().await {
        Ok(b) => b,
        Err(e) => {
            return err(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("storage read: {e}"),
            );
        }
    };

    info!(
        bytes = bytes.len(),
        "running InvoicePipeline::extract_from_bytes"
    );

    // Run the pipeline directly — no agent, no tool-calling
    let pipeline = InvoicePipeline::new();
    match pipeline.extract_from_bytes(&bytes).await {
        Ok(data) => (StatusCode::OK, Json(data)).into_response(),
        Err(e) => err(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("extraction failed: {e}"),
        ),
    }
}

fn err(code: StatusCode, msg: &str) -> Response {
    (code, Json(json!({ "error": msg }))).into_response()
}
