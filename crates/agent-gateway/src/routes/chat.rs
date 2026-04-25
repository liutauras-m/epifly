use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use axum::{extract::State, http::StatusCode, Extension, Json};
use rig::completion::Prompt;
use rig::providers::anthropic;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{info, instrument, warn};

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub model: Option<String>,
    pub messages: Vec<ChatMessage>,
    pub max_tokens: Option<u64>,
    #[allow(dead_code)]
    pub stream: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct ChatResponse {
    pub id: String,
    pub object: String,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Usage,
}

#[derive(Debug, Serialize)]
pub struct Choice {
    pub index: usize,
    pub message: ChatMessage,
    pub finish_reason: String,
}

#[derive(Debug, Serialize)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

#[instrument(skip(state, tenant, req), fields(
    tenant_id = tenant.0.tenant_id.as_str(),
    plan = %tenant.0.plan,
))]
pub async fn completions(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, (StatusCode, Json<Value>)> {
    // Per-tenant rate limit check
    if !state
        .rate_limiter
        .check(&tenant.0.tenant_id, tenant.0.plan.rate_limit_rpm())
    {
        warn!("rate limit hit");
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(
                json!({ "error": { "message": "rate limit exceeded", "type": "rate_limit_error" } }),
            ),
        ));
    }

    let model_id = req.model.as_deref().unwrap_or("claude-opus-4-7");
    let max_tokens = req
        .max_tokens
        .unwrap_or(4096)
        .min(tenant.0.plan.max_tokens());

    info!(
        model = model_id,
        messages = req.messages.len(),
        max_tokens,
        "chat completion"
    );

    let last_user = req
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.as_str())
        .unwrap_or("");

    let system = req
        .messages
        .iter()
        .find(|m| m.role == "system")
        .map(|m| m.content.clone());

    let client = anthropic::Client::from_env();
    let mut builder = client.agent(model_id).max_tokens(max_tokens);

    if let Some(sys) = system {
        builder = builder.preamble(&sys);
    }

    let agent = builder.build();
    let response_text = agent.prompt(last_user).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": { "message": e.to_string(), "type": "provider_error" } })),
        )
    })?;

    Ok(Json(ChatResponse {
        id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
        object: "chat.completion".into(),
        model: model_id.into(),
        choices: vec![Choice {
            index: 0,
            message: ChatMessage {
                role: "assistant".into(),
                content: response_text,
            },
            finish_reason: "stop".into(),
        }],
        usage: Usage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
        },
    }))
}
