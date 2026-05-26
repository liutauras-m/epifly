use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use agent_core::llm::types::LlmRequest;
use agent_core::{LlmRegistry, PlanLimits, map_rig_error};
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
use rig::OneOrMany;
use rig::completion::Message;
use rig::message::UserContent;
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
    /// Optional capability name to pin before semantic routing (PR 2.A).
    ///
    /// When set, tools from this capability are **prepended** before any semantic
    /// hits and survive truncation — the LLM is guaranteed to see them.
    /// Server-side validation: unknown or tenant-disabled capabilities are silently
    /// ignored (logged at WARN level); they never cause a 500.
    #[serde(default)]
    pub forced_capability: Option<String>,
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
        stream_response(Arc::clone(&state.llm), tenant, limits, req)
            .await
            .into_response()
    } else {
        match blocking_response(&state.llm, &tenant, limits, req).await {
            Ok(r) => r.into_response(),
            Err(e) => e.into_response(),
        }
    }
}

// ── Non-streaming ─────────────────────────────────────────────────────────────

async fn blocking_response(
    llm: &Arc<LlmRegistry>,
    _tenant: &ResolvedTenant,
    limits: PlanLimits,
    req: ChatRequest,
) -> Result<Json<ChatResponse>, HttpError> {
    let model_id = req.model.as_deref().unwrap_or("claude-opus-4-7");
    let max_tokens = req
        .max_tokens
        .unwrap_or(limits.max_tokens)
        .min(limits.max_tokens);

    info!(
        model = model_id,
        messages = req.messages.len(),
        max_tokens,
        "chat completion"
    );

    let provider = llm
        .resolve(model_id, None)
        .map_err(|e| HttpError::internal(e.to_string(), None))?;

    let messages = chat_messages_to_rig(&req.messages);
    let llm_req = LlmRequest::builder()
        .model(model_id.to_string())
        .messages(messages)
        .max_tokens(max_tokens as u32)
        .build();

    let resp = provider
        .complete(llm_req)
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
                content: resp.content,
            },
            finish_reason: resp.finish_reason.unwrap_or_else(|| "stop".into()),
        }],
        usage: Usage {
            prompt_tokens: resp
                .usage
                .as_ref()
                .map(|u| u.input_tokens as u64)
                .unwrap_or(0),
            completion_tokens: resp
                .usage
                .as_ref()
                .map(|u| u.output_tokens as u64)
                .unwrap_or(0),
            total_tokens: resp
                .usage
                .map(|u| (u.input_tokens + u.output_tokens) as u64)
                .unwrap_or(0),
        },
    }))
}

// ── Streaming SSE ─────────────────────────────────────────────────────────────

async fn stream_response(
    llm: Arc<LlmRegistry>,
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
        let max_tokens = req
            .max_tokens
            .unwrap_or(limits.max_tokens)
            .min(limits.max_tokens);
        let id = format!("chatcmpl-{}", Uuid::new_v4());

        let provider = match llm.resolve(&model_id, None) {
            Ok(p) => p,
            Err(e) => {
                let _ = tx
                    .send(Ok(
                        Event::default().data(json!({"error": e.to_string()}).to_string())
                    ))
                    .await;
                return;
            }
        };

        let messages = chat_messages_to_rig(&req.messages);
        let llm_req = LlmRequest::builder()
            .model(model_id.clone())
            .messages(messages)
            .max_tokens(max_tokens as u32)
            .build();

        match provider.stream(llm_req).await {
            Err(e) => {
                let _ = tx
                    .send(Ok(
                        Event::default().data(json!({"error": e.to_string()}).to_string())
                    ))
                    .await;
            }
            Ok(mut stream) => {
                while let Some(result) = stream.next().await {
                    match result {
                        Ok(chunk) => {
                            if !chunk.delta.is_empty() {
                                let chunk_json = json!({
                                    "id": id,
                                    "object": "chat.completion.chunk",
                                    "model": model_id,
                                    "choices": [{ "index": 0, "delta": { "content": chunk.delta }, "finish_reason": null }]
                                });
                                let _ = tx
                                    .send(Ok(Event::default().data(chunk_json.to_string())))
                                    .await;
                            }
                            if chunk.finish_reason.is_some() {
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
                        }
                        Err(e) => {
                            let _ = tx
                                .send(Ok(Event::default()
                                    .data(json!({"error": e.to_string()}).to_string())))
                                .await;
                        }
                    }
                }
            }
        }
    });

    Sse::new(ReceiverStream::new(rx))
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Convert `ChatMessage` list (role/content pairs) to Rig completion messages.
fn chat_messages_to_rig(msgs: &[ChatMessage]) -> Vec<Message> {
    msgs.iter()
        .filter_map(|m| match m.role.as_str() {
            "system" => Some(Message::System {
                content: m.content.clone(),
            }),
            "user" => Some(Message::User {
                content: OneOrMany::one(UserContent::text(m.content.clone())),
            }),
            _ => None,
        })
        .collect()
}
