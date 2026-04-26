/// POST /v1/agent/completions — Anthropic tool-use agent loop with optional thread memory.
///
/// Pass `"thread_id": "<ulid>"` in the request body to load history from Qdrant and
/// persist the new turn automatically.  Omit it for a stateless single-turn request.
use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use agent_core::capabilities::tool_executor::CapabilityExecutor;
use axum::{extract::State, http::StatusCode, Extension, Json};
use chrono::Utc;
use common::memory::thread::Message;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{info, instrument, warn, Span};
use uuid::Uuid;

const MAX_ROUNDS: usize = 5;

#[instrument(skip(state, tenant, req), fields(
    tenant_id  = tenant.0.tenant_id.as_str(),
    plan       = %tenant.0.plan,
    gen_ai.system = "anthropic",
    gen_ai.request.model = tracing::field::Empty,
    gen_ai.usage.input_tokens  = tracing::field::Empty,
    gen_ai.usage.output_tokens = tracing::field::Empty,
    thread_id  = tracing::field::Empty,
))]
pub async fn agent_completions(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    Json(req): Json<crate::routes::chat::ChatRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    // Rate limit
    if !state
        .rate_limiter
        .check(&tenant.0.tenant_id, tenant.0.plan.rate_limit_rpm())
    {
        warn!("rate limit hit");
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error": {"message": "rate limit exceeded", "type": "rate_limit_error"}})),
        ));
    }

    let api_key = std::env::var("ANTHROPIC_API_KEY").unwrap_or_default();
    let model_id = req
        .model
        .as_deref()
        .unwrap_or("claude-opus-4-7")
        .to_string();
    let max_tokens = req
        .max_tokens
        .unwrap_or(4096)
        .min(tenant.0.plan.max_tokens());

    Span::current().record("gen_ai.request.model", &model_id.as_str());

    // ── Thread memory ──────────────────────────────────────────────────────────
    let thread_id = req.thread_id.clone();
    if let Some(ref tid) = thread_id {
        Span::current().record("thread_id", tid.as_str());
    }

    let thread_store = Arc::clone(&state.thread_store);
    let tenant_id = tenant.0.tenant_id.clone();

    // Load thread history when thread_id provided
    let mut history_messages: Vec<Value> = if let Some(ref tid) = thread_id {
        match thread_store.messages(&tenant_id, tid).await {
            Ok(msgs) => msgs
                .iter()
                .map(|m| json!({"role": m.role, "content": m.content}))
                .collect(),
            Err(e) => {
                warn!(error = %e, thread_id = tid, "failed to load thread history");
                vec![]
            }
        }
    } else {
        vec![]
    };

    // Build tool definitions from the capability registry
    let tools: Vec<Value> = {
        let registry = state.registry.lock().unwrap();
        registry
            .all()
            .flat_map(CapabilityExecutor::tool_definitions)
            .collect()
    };

    let cards: Vec<_> = {
        let registry = state.registry.lock().unwrap();
        registry.all().cloned().collect()
    };

    // Merge history with new request messages (system handled separately)
    let new_messages: Vec<Value> = req
        .messages
        .iter()
        .filter(|m| m.role != "system")
        .map(|m| json!({"role": m.role, "content": m.content}))
        .collect();

    let system_content = req
        .messages
        .iter()
        .find(|m| m.role == "system")
        .map(|m| m.content.clone());

    // Inject thread summary as system context if present
    let effective_system = if let Some(ref tid) = thread_id {
        let summary = thread_store
            .get(&tenant_id, tid)
            .await
            .ok()
            .flatten()
            .and_then(|t| t.summary);

        match (system_content, summary) {
            (Some(sys), Some(sum)) => {
                Some(format!("{sys}\n\n[Conversation summary: {sum}]"))
            }
            (Some(sys), None) => Some(sys),
            (None, Some(sum)) => Some(format!("[Conversation summary: {sum}]")),
            (None, None) => None,
        }
    } else {
        system_content
    };

    // Persist user messages to thread before the agent loop
    if let Some(ref tid) = thread_id {
        for msg in req.messages.iter().filter(|m| m.role == "user") {
            let _ = thread_store
                .append(
                    &tenant_id,
                    tid,
                    Message {
                        role: "user".into(),
                        content: msg.content.clone(),
                        tool_calls: None,
                        timestamp: Utc::now(),
                        seq: 0, // seq is derived by the store
                    },
                )
                .await;
        }
    }

    history_messages.extend(new_messages);
    let mut messages = history_messages;

    let http = reqwest::Client::new();
    let mut tool_calls_made = 0usize;
    let mut total_input_tokens = 0u64;
    let mut total_output_tokens = 0u64;

    for round in 0..MAX_ROUNDS {
        let mut body = json!({
            "model": model_id,
            "max_tokens": max_tokens,
            "messages": messages,
            "tools": tools,
        });
        if let Some(ref sys) = effective_system {
            body["system"] = json!(sys);
        }

        info!(round, model = model_id, "agent loop iteration");

        let response: Value = http
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(|e| err500(format!("Anthropic request failed: {e}")))?
            .json()
            .await
            .map_err(|e| err500(format!("Response parse failed: {e}")))?;

        if let Some(err_type) = response.get("error") {
            return Err(err500(format!(
                "Anthropic error: {}",
                err_type["message"].as_str().unwrap_or("unknown")
            )));
        }

        total_input_tokens += response["usage"]["input_tokens"].as_u64().unwrap_or(0);
        total_output_tokens += response["usage"]["output_tokens"].as_u64().unwrap_or(0);

        let stop_reason = response["stop_reason"].as_str().unwrap_or("end_turn");
        let content = response["content"].as_array().cloned().unwrap_or_default();

        if stop_reason != "tool_use" {
            Span::current().record("gen_ai.usage.input_tokens", total_input_tokens);
            Span::current().record("gen_ai.usage.output_tokens", total_output_tokens);

            let text = content
                .iter()
                .find(|b| b["type"] == "text")
                .and_then(|b| b["text"].as_str())
                .unwrap_or("")
                .to_string();

            info!(
                input_tokens  = total_input_tokens,
                output_tokens = total_output_tokens,
                tool_calls    = tool_calls_made,
                "agent loop complete",
            );

            // Persist assistant reply to thread
            if let Some(ref tid) = thread_id {
                let _ = thread_store
                    .append(
                        &tenant_id,
                        tid,
                        Message {
                            role: "assistant".into(),
                            content: text.clone(),
                            tool_calls: None,
                            timestamp: Utc::now(),
                            seq: 0,
                        },
                    )
                    .await;

                // Auto-title the thread on first assistant reply
                maybe_set_title(&thread_store, &tenant_id, tid, &text).await;
            }

            return Ok(Json(json!({
                "id": format!("chatcmpl-{}", Uuid::new_v4()),
                "object": "chat.completion",
                "model": model_id,
                "choices": [{
                    "index": 0,
                    "message": {"role": "assistant", "content": text},
                    "finish_reason": "stop"
                }],
                "usage": {
                    "prompt_tokens":     total_input_tokens,
                    "completion_tokens": total_output_tokens,
                    "total_tokens":      total_input_tokens + total_output_tokens,
                },
                "tool_calls_made": tool_calls_made,
                "thread_id": thread_id,
            })));
        }

        // ── Tool use round ────────────────────────────────────────────────
        messages.push(json!({"role": "assistant", "content": content}));

        let mut tool_results: Vec<Value> = vec![];
        for block in &content {
            if block["type"] != "tool_use" {
                continue;
            }

            let call_id = block["id"].as_str().unwrap_or("").to_string();
            let tool_name = block["name"].as_str().unwrap_or("");
            let tool_input = &block["input"];

            info!(round, tool = tool_name, "executing tool");
            tool_calls_made += 1;

            let result_content =
                match resolve_and_invoke(&cards, tool_name, tool_input, &tenant).await {
                    Ok(v) => v.to_string(),
                    Err(e) => {
                        warn!(tool = tool_name, error = %e, "tool invocation failed");
                        format!("Error: {e}")
                    }
                };

            tool_results.push(json!({
                "type": "tool_result",
                "tool_use_id": call_id,
                "content": result_content
            }));
        }

        messages.push(json!({"role": "user", "content": tool_results}));
    }

    Err(err500(format!(
        "Exceeded {MAX_ROUNDS} tool call rounds without a final response"
    )))
}

/// Set the thread title from the first ~60 chars of the assistant's first reply,
/// but only once (when current title is None).
async fn maybe_set_title(
    store: &Arc<dyn common::memory::ThreadStore>,
    tenant_id: &str,
    thread_id: &str,
    assistant_text: &str,
) {
    let already_titled = store
        .get(tenant_id, thread_id)
        .await
        .ok()
        .flatten()
        .map(|t| t.title.is_some())
        .unwrap_or(false);

    if !already_titled && !assistant_text.is_empty() {
        let title: String = assistant_text.chars().take(60).collect();
        let _ = store.set_title(tenant_id, thread_id, title).await;
    }
}

async fn resolve_and_invoke(
    cards: &[agent_core::capabilities::card::CapabilityCard],
    full_tool_name: &str,
    input: &Value,
    tenant: &ResolvedTenant,
) -> anyhow::Result<Value> {
    let (cap_name, tool_name) = full_tool_name
        .split_once("__")
        .ok_or_else(|| anyhow::anyhow!("invalid tool name format: {full_tool_name}"))?;

    let card = cards
        .iter()
        .find(|c| c.manifest.name == cap_name)
        .ok_or_else(|| anyhow::anyhow!("capability not found: {cap_name}"))?;

    CapabilityExecutor::invoke(card, tool_name, input, Some(&tenant.0)).await
}

fn err500(msg: String) -> (StatusCode, Json<Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": {"message": msg, "type": "server_error"}})),
    )
}
