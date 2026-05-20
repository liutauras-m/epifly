use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use agent_core::{PlanLimits, map_rig_error};
use axum::{
    Extension, Json,
    extract::State,
    response::{
        IntoResponse, Response,
        sse::{Event, Sse},
    },
};
use common::error::HttpError;
use futures::StreamExt;
use rig::client::ProviderClient;
use rig::client::completion::CompletionClient;
use rig::completion::Prompt;
use rig::providers::anthropic;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{info, instrument, warn};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct ChatRequest {
    pub model: Option<String>,
    pub messages: Vec<ChatMessage>,
    pub max_tokens: Option<u64>,
    pub stream: Option<bool>,
    /// Optional ULID — when provided, loads history from Postgres and persists this turn.
    pub thread_id: Option<String>,
    /// Optional workspace node ULID — when provided, injects folder+conversation context.
    pub workspace_node_id: Option<String>,
    /// Maximum tool-call rounds for this request. Capped at the tenant plan limit.
    /// Reserved: use /v1/agent/completions for full agentic context.
    pub max_turns: Option<u32>,
    /// Pre-resolved Anthropic content blocks for attached files (images, documents, text).
    /// Set by the UI stream handler after fetching bytes from object storage.
    #[serde(default)]
    pub attachment_content: Vec<Value>,
}

#[derive(Debug, Deserialize, Serialize, Clone, ToSchema)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ChatResponse {
    pub id: String,
    pub object: String,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Usage,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Choice {
    pub index: usize,
    pub message: ChatMessage,
    pub finish_reason: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

#[utoipa::path(
    post,
    path = "/v1/chat/completions",
    request_body = ChatRequest,
    responses(
        (status = 200, description = "Chat completion (JSON or SSE stream)", body = ChatResponse),
        (status = 429, description = "Rate limit exceeded"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer_auth" = [])),
    tag = "chat",
)]
#[instrument(skip(state, tenant, req), fields(
    tenant_id = tenant.0.tenant_id.as_str(),
    plan      = %tenant.0.plan,
))]
pub async fn completions(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    Extension(limits): Extension<PlanLimits>,
    Json(req): Json<ChatRequest>,
) -> Response {
    // Per-tenant rate limit
    if !state
        .rate_limiter
        .check(&tenant.0.tenant_id, limits.rate_limit_rpm)
    {
        warn!("rate limit hit");
        return HttpError::rate_limit(None).into_response();
    }

    if req.stream.unwrap_or(false) {
        stream_response(tenant, limits, req).await.into_response()
    } else {
        match blocking_response(&state, &tenant, limits, req).await {
            Ok(r) => r.into_response(),
            Err(e) => e.into_response(),
        }
    }
}

// ── Non-streaming ─────────────────────────────────────────────────────────────

async fn blocking_response(
    _state: &Arc<AppState>,
    _tenant: &ResolvedTenant,
    limits: PlanLimits,
    req: ChatRequest,
) -> Result<Json<ChatResponse>, HttpError> {
    let model_id = req.model.as_deref().unwrap_or("claude-opus-4-7");
    let max_tokens = req.max_tokens.unwrap_or(limits.max_tokens).min(limits.max_tokens);

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

    let client = anthropic::Client::from_env().expect("ANTHROPIC_API_KEY must be set");
    let mut builder = client.agent(model_id).max_tokens(max_tokens);
    if let Some(sys) = system {
        builder = builder.preamble(&sys);
    }

    let agent = builder.build();
    let text = agent
        .prompt(last_user)
        .await
        .map_err(|e| map_rig_error(e.to_string()))?;

    Ok(Json(ChatResponse {
        id: format!("chatcmpl-{}", Uuid::new_v4()),
        object: "chat.completion".into(),
        model: model_id.into(),
        choices: vec![Choice {
            index: 0,
            message: ChatMessage {
                role: "assistant".into(),
                content: text,
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

// ── Streaming SSE ─────────────────────────────────────────────────────────────

async fn stream_response(
    _tenant: ResolvedTenant,
    limits: PlanLimits,
    req: ChatRequest,
) -> Sse<ReceiverStream<Result<Event, Infallible>>> {
    let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(64);

    tokio::spawn(async move {
        let model_id = req
            .model
            .as_deref()
            .unwrap_or("claude-opus-4-7")
            .to_string();
        let max_tokens = req.max_tokens.unwrap_or(limits.max_tokens).min(limits.max_tokens);
        let id = format!("chatcmpl-{}", Uuid::new_v4());
        let api_key = std::env::var("ANTHROPIC_API_KEY").unwrap_or_default();

        // Build Anthropic messages (skip system role, send separately)
        let messages: Vec<Value> = req
            .messages
            .iter()
            .filter(|m| m.role != "system")
            .map(|m| json!({"role": m.role, "content": m.content}))
            .collect();

        let mut body = json!({
            "model": model_id,
            "messages": messages,
            "max_tokens": max_tokens,
            "stream": true,
        });
        if let Some(sys) = req.messages.iter().find(|m| m.role == "system") {
            body["system"] = json!(sys.content);
        }

        let http = reqwest::Client::new();
        let resp = http
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await;

        match resp {
            Err(e) => {
                let _ = tx
                    .send(Ok(
                        Event::default().data(json!({"error": e.to_string()}).to_string())
                    ))
                    .await;
            }
            Ok(response) => {
                let mut byte_stream = response.bytes_stream();
                let mut buf = String::new();

                while let Some(chunk) = byte_stream.next().await {
                    let Ok(bytes) = chunk else { break };
                    buf.push_str(&String::from_utf8_lossy(&bytes));

                    // SSE events are separated by \n\n
                    while let Some(pos) = buf.find("\n\n") {
                        let event_block = buf[..pos].to_string();
                        buf = buf[pos + 2..].to_string();

                        for line in event_block.lines() {
                            let Some(data) = line.strip_prefix("data: ") else {
                                continue;
                            };
                            if data == "[DONE]" {
                                break;
                            }
                            let Ok(ev) = serde_json::from_str::<Value>(data) else {
                                continue;
                            };

                            match ev["type"].as_str().unwrap_or("") {
                                "content_block_delta" => {
                                    let Some(text) = ev["delta"]["text"].as_str() else {
                                        continue;
                                    };
                                    let chunk_json = json!({
                                        "id": id,
                                        "object": "chat.completion.chunk",
                                        "model": model_id,
                                        "choices": [{ "index": 0, "delta": { "content": text }, "finish_reason": null }]
                                    });
                                    let _ = tx
                                        .send(Ok(Event::default().data(chunk_json.to_string())))
                                        .await;
                                }
                                "message_stop" => {
                                    let done_json = json!({
                                        "id": id,
                                        "object": "chat.completion.chunk",
                                        "model": model_id,
                                        "choices": [{ "index": 0, "delta": {}, "finish_reason": "stop" }]
                                    });
                                    let _ = tx
                                        .send(Ok(Event::default().data(done_json.to_string())))
                                        .await;
                                    let _ = tx.send(Ok(Event::default().data("[DONE]"))).await;
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    });

    Sse::new(ReceiverStream::new(rx))
}
