//! Chat streaming handler — delegates to the existing agent loop in-process.
//!
//! Accepts a JSON body `{ message, thread_id?, model?, workspace_node_id?, attachment_ids? }`
//! from the UI composer, resolves any attached file tokens into Anthropic content blocks,
//! constructs a `ChatRequest`, and proxies SSE chunks straight to the browser.

use crate::auth::ResolvedTenant;
use crate::routes::agent;
use crate::routes::chat::{ChatMessage, ChatRequest};
use crate::state::AppState;
use agent_core::TenantStorage;
use axum::{
    Extension, Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use common::error::HttpError;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Deserialize)]
pub struct UiChatBody {
    pub message: String,
    #[serde(default)]
    pub thread_id: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub workspace_node_id: Option<String>,
    #[serde(default)]
    pub attachment_ids: Vec<String>,
    /// Optional capability name to pin for this turn (PR 2.A).
    /// Sent by the UI "Invoke in current workspace" button.
    #[serde(default)]
    pub forced_capability: Option<String>,
}

pub async fn ui_stream(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    Json(body): Json<UiChatBody>,
) -> Response {
    let trimmed = body.message.trim();
    if trimmed.is_empty() && body.attachment_ids.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "message required"})),
        )
            .into_response();
    }

    if !state
        .rate_limiter
        .check(&tenant.0.tenant_id, tenant.0.plan.limits().rate_limit_rpm)
    {
        return HttpError::rate_limit(None).into_response();
    }

    // Resolve TenantStorage once; passed into attachment helpers for ownership checks.
    let storage = if let Some(factory) = state.tenant_storage.as_ref() {
        factory.for_tenant(tenant.0.tenant_id.as_str()).await.ok()
    } else {
        None
    };

    let attachment_content = if !body.attachment_ids.is_empty() {
        resolve_attachments(&body.attachment_ids, storage.as_ref()).await
    } else {
        vec![]
    };

    let attachment_hint = if !body.attachment_ids.is_empty() {
        build_attachment_hint(&body.attachment_ids, storage.as_ref())
    } else {
        String::new()
    };

    let effective_message = if attachment_hint.is_empty() {
        trimmed.to_string()
    } else if trimmed.is_empty() {
        attachment_hint
    } else {
        format!("{trimmed}\n\n{attachment_hint}")
    };

    let req = ChatRequest {
        model: body.model,
        messages: vec![ChatMessage {
            role: "user".into(),
            content: effective_message,
        }],
        max_tokens: Some(2048),
        stream: Some(true),
        thread_id: body.thread_id,
        workspace_node_id: body.workspace_node_id,
        max_turns: None,
        attachment_content,
        forced_capability: body.forced_capability,
    };

    let limits = tenant.0.plan.limits();
    agent::stream_agent(state, tenant, limits, req)
        .await
        .into_response()
}

/// Tell the model which attachments it can pass to vision tools. `image_path` is the
/// object key in the tenant's storage — the only form vision tools accept.
fn build_attachment_hint(attachment_ids: &[String], storage: Option<&TenantStorage>) -> String {
    let lines: Vec<String> = attachment_ids
        .iter()
        .filter(|key| storage.is_some_and(|s| s.owns_object_key(key)))
        .map(|key| {
            let filename = key.rsplit('/').next().unwrap_or("file");
            format!("- {filename} (image_path: {key})")
        })
        .collect();
    if lines.is_empty() {
        return String::new();
    }
    format!(
        "[Attached files — pass image_path directly to invoice-processing__extract_invoice or ocr-service__extract_text]\n{}",
        lines.join("\n")
    )
}

// ── Attachment resolution ─────────────────────────────────────────────────────

/// Fetch each upload token from object storage and convert to Anthropic content blocks.
///
/// - Images (jpeg/png/gif/webp): `{"type":"image","source":{"type":"base64",...}}`
/// - Text files (txt/md/json/csv/…): `{"type":"document","source":{"type":"text",...}}`
/// - PDFs: `{"type":"document","source":{"type":"base64","media_type":"application/pdf",...}}`
/// - Everything else: a text block describing the file
async fn resolve_attachments(
    attachment_ids: &[String],
    storage: Option<&TenantStorage>,
) -> Vec<Value> {
    let Some(storage) = storage else {
        return vec![];
    };

    let mut blocks = Vec::with_capacity(attachment_ids.len());

    for object_key in attachment_ids {
        // Attachment ids are object keys; read with the tenant's own credentials.
        let filename = object_key
            .split('/')
            .next_back()
            .unwrap_or("file")
            .to_string();
        let ct = content_type_from_filename(&filename);
        let bytes = match storage.get_owned_object(object_key).await {
            Ok(b) => b,
            Err(_) => continue,
        };

        let block = match ct {
            ct if is_image(ct) => json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": ct,
                    "data": B64.encode(&bytes)
                }
            }),
            "application/pdf" => json!({
                "type": "document",
                "source": {
                    "type": "base64",
                    "media_type": "application/pdf",
                    "data": B64.encode(&bytes)
                },
                "title": filename
            }),
            _ if is_text_like(ct) => match std::str::from_utf8(&bytes) {
                Ok(text) => json!({
                    "type": "document",
                    "source": {
                        "type": "text",
                        "media_type": "text/plain",
                        "data": text
                    },
                    "title": filename
                }),
                Err(_) => json!({
                    "type": "text",
                    "text": format!("[Attached file: {filename} ({} bytes) — could not decode as UTF-8]", bytes.len())
                }),
            },
            _ => json!({
                "type": "text",
                "text": format!("[Attached binary file: {filename} ({} bytes)]", bytes.len())
            }),
        };

        blocks.push(block);
    }

    blocks
}

fn content_type_from_filename(name: &str) -> &'static str {
    match name
        .rsplit('.')
        .next()
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("pdf") => "application/pdf",
        Some(
            "txt" | "md" | "markdown" | "csv" | "json" | "yaml" | "yml" | "toml" | "xml" | "html"
            | "htm" | "css" | "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "rb" | "go" | "java"
            | "c" | "cpp" | "h" | "sh" | "sql" | "log",
        ) => "text/plain",
        _ => "application/octet-stream",
    }
}

fn is_image(ct: &str) -> bool {
    matches!(ct, "image/jpeg" | "image/png" | "image/gif" | "image/webp")
}

fn is_text_like(ct: &str) -> bool {
    ct.starts_with("text/") || matches!(ct, "application/json" | "application/xml")
}
