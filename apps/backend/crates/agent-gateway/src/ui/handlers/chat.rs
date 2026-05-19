//! Chat streaming handler — delegates to the existing agent loop in-process.
//!
//! Accepts a JSON body `{ message, thread_id?, model?, workspace_node_id?, attachment_ids? }`
//! from the UI composer, resolves any attached file tokens into Anthropic content blocks,
//! constructs a `ChatRequest`, and proxies SSE chunks straight to the browser.

use crate::mw::tenant::ResolvedTenant;
use crate::routes::agent;
use crate::routes::chat::{ChatMessage, ChatRequest};
use crate::state::AppState;
use crate::ui::session::SessionUser;
use axum::{
    http::HeaderMap,
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use common::error::HttpError;
use object_store::path::Path as OsPath;
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
}

pub async fn ui_stream(
    State(state): State<Arc<AppState>>,
    user: SessionUser,
    headers: HeaderMap,
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

    let tenant = ResolvedTenant(user.tenant_context());

    if !state
        .rate_limiter
        .check(&tenant.0.tenant_id, tenant.0.plan.rate_limit_rpm())
    {
        return HttpError::rate_limit(None).into_response();
    }

    let attachment_content = if !body.attachment_ids.is_empty() {
        resolve_attachments(&state, &body.attachment_ids, tenant.0.tenant_id.as_str()).await
    } else {
        vec![]
    };

    let attachment_hint = if !body.attachment_ids.is_empty() {
        build_attachment_hint(&state, &headers, &body.attachment_ids, tenant.0.tenant_id.as_str())
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
        model: body.model.or_else(|| Some("claude-opus-4-7".into())),
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
    };

    agent::stream_agent(state, tenant, req)
        .await
        .into_response()
}

fn build_attachment_hint(
    state: &Arc<AppState>,
    headers: &HeaderMap,
    attachment_ids: &[String],
    tenant_id: &str,
) -> String {
    let origin = request_origin(headers);
    let expected_prefix = format!("tenants/{tenant_id}/");

    let mut lines = Vec::new();
    for object_key in attachment_ids {
        if !object_key.starts_with(&expected_prefix) {
            continue;
        }
        let filename = object_key.split('/').next_back().unwrap_or("file");
        let encoded_key = object_key
            .chars()
            .flat_map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '~' || c == '/' {
                    vec![c]
                } else {
                    format!("%{:02X}", c as u32).chars().collect()
                }
            })
            .collect::<String>();
        lines.push(format!(
            "- {filename} (image_path: {origin}/ui/files/download?key={encoded_key})"
        ));
    }

    if lines.is_empty() {
        return String::new();
    }

    format!(
        "[Attached files — pass image_path directly to invoice-processing__extract_invoice or ocr-service__extract_text]\n{}",
        lines.join("\n")
    )
}

fn request_origin(headers: &HeaderMap) -> String {
    let proto = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("http");

    let host = headers
        .get("x-forwarded-host")
        .or_else(|| headers.get("host"))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost:8080");

    format!("{proto}://{host}")
}

// ── Attachment resolution ─────────────────────────────────────────────────────

/// Fetch each upload token from object storage and convert to Anthropic content blocks.
///
/// - Images (jpeg/png/gif/webp): `{"type":"image","source":{"type":"base64",...}}`
/// - Text files (txt/md/json/csv/…): `{"type":"document","source":{"type":"text",...}}`
/// - PDFs: `{"type":"document","source":{"type":"base64","media_type":"application/pdf",...}}`
/// - Everything else: a text block describing the file
async fn resolve_attachments(
    state: &Arc<AppState>,
    attachment_ids: &[String],
    tenant_id: &str,
) -> Vec<Value> {
    let Some(store) = state.file_store.as_ref() else {
        return vec![];
    };

    let expected_prefix = format!("tenants/{tenant_id}/");
    let mut blocks = Vec::with_capacity(attachment_ids.len());

    for object_key in attachment_ids {
        // attachment_ids are now object keys directly (no UUID token lookup)
        if !object_key.starts_with(&expected_prefix) {
            continue;
        }
        let object_key = object_key.clone();

        let filename = object_key.split('/').next_back().unwrap_or("file").to_string();
        let ct = content_type_from_filename(&filename);

        let os_path = OsPath::from(object_key.as_str());
        let get_result = match store.get(&os_path).await {
            Ok(r) => r,
            Err(_) => continue,
        };
        let bytes = match get_result.bytes().await {
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
    match name.rsplit('.').next().map(|e| e.to_ascii_lowercase()).as_deref() {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("pdf") => "application/pdf",
        Some(
            "txt" | "md" | "markdown" | "csv" | "json" | "yaml" | "yml" | "toml" | "xml"
            | "html" | "htm" | "css" | "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "rb"
            | "go" | "java" | "c" | "cpp" | "h" | "sh" | "sql" | "log",
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
