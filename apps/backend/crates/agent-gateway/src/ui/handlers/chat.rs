//! Chat streaming handler — delegates to the existing agent loop in-process.
//!
//! Accepts a JSON body `{ message, thread_id?, model? }` from the UI composer,
//! constructs a `ChatRequest`, and proxies SSE chunks straight to the browser.

use crate::mw::tenant::ResolvedTenant;
use crate::routes::agent;
use crate::routes::chat::{ChatMessage, ChatRequest};
use crate::state::AppState;
use crate::ui::session::SessionUser;
use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use common::error::HttpError;
use serde::Deserialize;
use serde_json::json;
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
}

pub async fn ui_stream(
    State(state): State<Arc<AppState>>,
    user: SessionUser,
    Json(body): Json<UiChatBody>,
) -> Response {
    let trimmed = body.message.trim();
    if trimmed.is_empty() {
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

    let req = ChatRequest {
        model: body.model.or_else(|| Some("claude-opus-4-7".into())),
        messages: vec![ChatMessage {
            role: "user".into(),
            content: trimmed.into(),
        }],
        max_tokens: Some(2048),
        stream: Some(true),
        thread_id: body.thread_id,
        workspace_node_id: body.workspace_node_id,
        max_turns: None,
    };

    agent::stream_agent(state, tenant, req)
        .await
        .into_response()
}
