/// POST /v1/agent/completions — Anthropic tool-use agent loop with optional thread memory.
///
/// Supports both blocking (default) and streaming (`"stream": true`) modes.
/// When streaming, emits OpenAI-compatible SSE chunks plus extra `tool_call_start` /
/// `tool_call_result` event types so clients can follow tool execution in real-time.
///
/// Pass `"thread_id": "<ulid>"` to load history from Postgres and persist the turn.
/// Pass `"max_turns": N` to override the default tool-call rounds (capped by plan tier).
use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use agent_core::ContextBuilder;
use agent_core::map_rig_error;
use agent_core::tools::executor::ToolExecutor;
use axum::{
    Extension, Json,
    extract::State,
    response::{
        IntoResponse, Response,
        sse::{Event, Sse},
    },
};
use chrono::Utc;
use common::error::HttpError;
use common::memory::thread::Message;
use common::metrics;
use futures::StreamExt;
use serde_json::{Value, json};
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{Span, info, instrument, warn};
use ulid::Ulid as _Ulid;
use uuid::Uuid;

#[utoipa::path(
    post,
    path = "/v1/agent/completions",
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Agent completion (JSON or SSE stream)", body = serde_json::Value),
        (status = 429, description = "Rate limit exceeded"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer_auth" = [])),
    tag = "agent",
)]
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
) -> Response {
    if !state
        .rate_limiter
        .check(&tenant.0.tenant_id, tenant.0.plan.rate_limit_rpm())
    {
        warn!("rate limit hit");
        return HttpError::rate_limit(None).into_response();
    }

    if req.stream.unwrap_or(false) {
        stream_agent(state, tenant, req).await.into_response()
    } else {
        match blocking_agent(state, tenant, req).await {
            Ok(v) => Json(v).into_response(),
            Err(e) => e.into_response(),
        }
    }
}

// ── Shared setup ─────────────────────────────────────────────────────────────

struct AgentCtx {
    api_key: String,
    model_id: String,
    max_tokens: u64,
    /// Effective maximum tool-call rounds: min(request.max_turns, plan.max_turns).
    max_rounds: usize,
    thread_id: Option<String>,
    tenant_id: String,
    tools: Vec<Value>,
    messages: Vec<Value>,
    effective_system: Option<String>,
    /// Parsed workspace node ULID, used to index chat content for search.
    workspace_node_id: Option<_Ulid>,
}

async fn build_ctx(
    state: &Arc<AppState>,
    tenant: &ResolvedTenant,
    req: &crate::routes::chat::ChatRequest,
) -> Result<AgentCtx, HttpError> {
    let api_key = std::env::var("ANTHROPIC_API_KEY").unwrap_or_default();
    if api_key.trim().is_empty() {
        return Err(err500(
            "ANTHROPIC_API_KEY is not configured; set it before starting agent-gateway".into(),
        ));
    }
    let model_id = req
        .model
        .as_deref()
        .unwrap_or("claude-opus-4-7")
        .to_string();
    let max_tokens = req
        .max_tokens
        .unwrap_or(4096)
        .min(tenant.0.plan.max_tokens());

    Span::current().record("gen_ai.request.model", model_id.as_str());

    let tenant_id = tenant.0.tenant_id.to_string();
    let thread_store = Arc::clone(&state.thread_store);

    // Effective max_rounds: honour request value but cap at plan tier limit.
    let plan_max = tenant.0.plan.max_turns() as usize;
    let max_rounds = req
        .max_turns
        .map(|s| (s as usize).min(plan_max))
        .unwrap_or(plan_max);

    // Resolve effective thread_id via ConversationService:
    //   1. Explicit `thread_id` on the request always wins.
    //   2. If `workspace_node_id` is provided, resolve (or lazily create) the
    //      thread bound to that node via ConversationService.
    //   3. Otherwise no thread (transient turn).
    let thread_id: Option<String> = if let Some(tid) = req.thread_id.clone() {
        Some(tid)
    } else if let Some(ref node_id_str) = req.workspace_node_id {
        match node_id_str.parse::<_Ulid>() {
            Ok(node_id) => {
                match state
                    .conversation_service
                    .resolve_for_node(&tenant.0, node_id)
                    .await
                {
                    Ok(Some(tid)) => Some(tid.to_string()),
                    Ok(None) => None,
                    Err(e) => {
                        warn!(error = %e, "ConversationService::resolve_for_node failed");
                        None
                    }
                }
            }
            Err(_) => None,
        }
    } else {
        None
    };

    if let Some(ref tid) = thread_id {
        Span::current().record("thread_id", tid.as_str());
    }

    // Load thread history
    let mut history: Vec<Value> = if let Some(ref tid) = thread_id {
        match thread_store.messages(&tenant_id, tid).await {
            Ok(msgs) => msgs
                .iter()
                .map(|m| json!({"role": m.role, "content": m.content}))
                .collect(),
            Err(e) => {
                warn!(error = %e, "failed to load thread history");
                vec![]
            }
        }
    } else {
        vec![]
    };

    let tools: Vec<Value> = {
        let registry = state.registry.lock().unwrap();
        registry
            .all_enabled()
            .flat_map(ToolExecutor::tool_definitions)
            .collect()
    };
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

    let base_system = if let Some(ref tid) = thread_id {
        let summary = thread_store
            .get(&tenant_id, tid)
            .await
            .ok()
            .flatten()
            .and_then(|t| t.summary);
        match (system_content, summary) {
            (Some(sys), Some(sum)) => Some(format!("{sys}\n\n[Conversation summary: {sum}]")),
            (Some(sys), None) => Some(sys),
            (None, Some(sum)) => Some(format!("[Conversation summary: {sum}]")),
            (None, None) => None,
        }
    } else {
        system_content
    };

    // Inject workspace context when workspace_node_id is provided
    let effective_system = if let Some(ref node_id_str) = req.workspace_node_id {
        if let Ok(node_id) = node_id_str.parse::<_Ulid>() {
            let ctx_builder = ContextBuilder::new(
                Arc::clone(&state.workspace_store),
                Arc::clone(&state.workspace_content),
            );
            let ws_ctx = ctx_builder.build_for_node(&tenant.0, node_id, 6000).await;
            if ws_ctx.is_empty() {
                base_system
            } else {
                Some(match base_system {
                    Some(existing) => format!("{existing}\n\n{ws_ctx}"),
                    None => ws_ctx,
                })
            }
        } else {
            base_system
        }
    } else {
        base_system
    };

    // Persist incoming user messages to thread
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
                        seq: 0,
                    },
                )
                .await;
        }
    }

    history.extend(new_messages);

    // Parse workspace_node_id for later content indexing.
    let workspace_node_id = req
        .workspace_node_id
        .as_ref()
        .and_then(|s| s.parse::<_Ulid>().ok());

    Ok(AgentCtx {
        api_key,
        model_id,
        max_tokens,
        max_rounds,
        thread_id,
        tenant_id,
        tools,
        messages: history,
        effective_system,
        workspace_node_id,
    })
}

// ── Blocking path ───────────────────────────────────────────────────

async fn blocking_agent(
    state: Arc<AppState>,
    tenant: ResolvedTenant,
    req: crate::routes::chat::ChatRequest,
) -> Result<Value, HttpError> {
    let ctx = build_ctx(&state, &tenant, &req).await?;

    let AgentCtx {
        api_key,
        model_id,
        max_tokens,
        max_rounds,
        thread_id,
        tenant_id,
        tools,
        mut messages,
        effective_system,
        workspace_node_id,
    } = ctx;

    let thread_store = Arc::clone(&state.thread_store);
    let http = reqwest::Client::new();
    let mut tool_calls_made = 0usize;
    let mut total_input = 0u64;
    let mut total_output = 0u64;

    for round in 0..max_rounds {
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

        if let Some(err) = response.get("error") {
            return Err(err500(format!(
                "Anthropic error: {}",
                err["message"].as_str().unwrap_or("unknown")
            )));
        }

        total_input += response["usage"]["input_tokens"].as_u64().unwrap_or(0);
        total_output += response["usage"]["output_tokens"].as_u64().unwrap_or(0);

        let stop_reason = response["stop_reason"].as_str().unwrap_or("end_turn");
        let content = response["content"].as_array().cloned().unwrap_or_default();

        if stop_reason != "tool_use" {
            Span::current().record("gen_ai.usage.input_tokens", total_input);
            Span::current().record("gen_ai.usage.output_tokens", total_output);

            // Emit metrics for this completion turn.
            let model_label = [metrics::kv("model", model_id.as_str())];
            metrics::llm_requests().add(1, &model_label);
            metrics::llm_input_tokens().record(total_input, &model_label);
            metrics::llm_output_tokens().record(total_output, &model_label);

            let text = content
                .iter()
                .find(|b| b["type"] == "text")
                .and_then(|b| b["text"].as_str())
                .unwrap_or("")
                .to_string();

            info!(
                input_tokens = total_input,
                output_tokens = total_output,
                tool_calls = tool_calls_made,
                "agent loop complete"
            );

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
                maybe_set_title(&thread_store, &tenant_id, tid, &text).await;
            }

            // Index recent thread messages into the workspace node so the full
            // conversation history is searchable (not just the latest turn).
            if let (Some(node_id), Some(tid)) = (workspace_node_id, thread_id.as_ref()) {
                let recent = thread_store
                    .messages(&tenant_id, tid)
                    .await
                    .unwrap_or_default();
                let snippet: String = recent
                    .iter()
                    .rev()
                    .take(30)
                    .rev()
                    .map(|m| format!("{}: {}", m.role, m.content))
                    .collect::<Vec<_>>()
                    .join("\n\n");
                let _ = state
                    .workspace_store
                    .index_content(&tenant_id, node_id, &snippet)
                    .await;
            }

            return Ok(json!({
                "id": format!("chatcmpl-{}", Uuid::new_v4()),
                "object": "chat.completion",
                "model": model_id,
                "choices": [{"index": 0, "message": {"role": "assistant", "content": text}, "finish_reason": "stop"}],
                "usage": {
                    "prompt_tokens":     total_input,
                    "completion_tokens": total_output,
                    "total_tokens":      total_input + total_output,
                },
                "tool_calls_made": tool_calls_made,
                "thread_id": thread_id,
            }));
        }

        // Tool use round
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

            let result = match resolve_and_invoke(&state, tool_name, tool_input, &tenant).await {
                Ok(v) => v.to_string(),
                Err(e) => {
                    warn!(tool = tool_name, error = %e, "tool invocation failed");
                    format!("Error: {e}")
                }
            };

            tool_results.push(json!({
                "type": "tool_result",
                "tool_use_id": call_id,
                "content": result
            }));
        }

        messages.push(json!({"role": "user", "content": tool_results}));
    }

    Err(err500(format!(
        "Exceeded {max_rounds} tool call rounds without a final response"
    )))
}

// ── Streaming path ────────────────────────────────────────────────────────────

pub async fn stream_agent(
    state: Arc<AppState>,
    tenant: ResolvedTenant,
    req: crate::routes::chat::ChatRequest,
) -> Sse<ReceiverStream<Result<Event, Infallible>>> {
    let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(128);

    tokio::spawn(async move {
        let ctx = match build_ctx(&state, &tenant, &req).await {
            Ok(c) => c,
            Err(e) => {
                let message = e.body.message.as_str();
                emit_stream_error(&tx, "chatcmpl-init-error", "claude-opus-4-7", message, None)
                    .await;
                return;
            }
        };

        let AgentCtx {
            api_key,
            model_id,
            max_tokens,
            max_rounds,
            thread_id,
            tenant_id,
            tools,
            mut messages,
            effective_system,
            workspace_node_id,
        } = ctx;

        let thread_store = Arc::clone(&state.thread_store);
        let http = reqwest::Client::new();
        let completion_id = format!("chatcmpl-{}", Uuid::new_v4());
        let mut tool_calls_made = 0usize;
        let mut total_input = 0u64;
        let mut total_output = 0u64;
        let mut full_assistant_text = String::new();

        'rounds: for round in 0..max_rounds {
            let mut body = json!({
                "model": model_id,
                "max_tokens": max_tokens,
                "messages": messages,
                "tools": tools,
                "stream": true,
            });
            if let Some(ref sys) = effective_system {
                body["system"] = json!(sys);
            }

            let resp = http
                .post("https://api.anthropic.com/v1/messages")
                .header("x-api-key", &api_key)
                .header("anthropic-version", "2023-06-01")
                .json(&body)
                .send()
                .await;

            let response = match resp {
                Ok(r) => r,
                Err(e) => {
                    emit_stream_error(
                        &tx,
                        &completion_id,
                        &model_id,
                        &format!("upstream request failed: {e}"),
                        thread_id.as_deref(),
                    )
                    .await;
                    return;
                }
            };

            if !response.status().is_success() {
                let status = response.status();
                let raw = response.text().await.unwrap_or_default();
                let upstream = serde_json::from_str::<Value>(&raw)
                    .ok()
                    .and_then(|v| {
                        v.get("error")
                            .and_then(|e| e.get("message"))
                            .and_then(|m| m.as_str())
                            .map(|s| s.to_string())
                            .or_else(|| {
                                v.get("error")
                                    .and_then(|e| e.as_str())
                                    .map(|s| s.to_string())
                            })
                    })
                    .unwrap_or_else(|| raw.clone());

                let message = format!(
                    "upstream returned {} {}{}",
                    status.as_u16(),
                    status.canonical_reason().unwrap_or(""),
                    if upstream.is_empty() {
                        "".to_string()
                    } else {
                        format!(": {upstream}")
                    }
                );

                emit_stream_error(
                    &tx,
                    &completion_id,
                    &model_id,
                    &message,
                    thread_id.as_deref(),
                )
                .await;
                return;
            }

            let mut byte_stream = response.bytes_stream();
            let mut buf = String::new();

            // Accumulated state across SSE events for this round
            let mut stop_reason = String::new();
            // tool blocks: index → (id, name, accumulated_json)
            let mut tool_blocks: std::collections::HashMap<usize, (String, String, String)> =
                std::collections::HashMap::new();
            // full assistant content array for the next messages turn
            let mut assistant_content: Vec<Value> = vec![];
            // current text block content
            let mut current_text = String::new();

            while let Some(chunk) = byte_stream.next().await {
                let Ok(bytes) = chunk else { break };
                buf.push_str(&String::from_utf8_lossy(&bytes));

                while let Some(pos) = buf.find("\n\n") {
                    let block = buf[..pos].to_string();
                    buf = buf[pos + 2..].to_string();

                    for line in block.lines() {
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
                            "message_start" => {
                                total_input +=
                                    ev["message"]["usage"]["input_tokens"].as_u64().unwrap_or(0);
                            }

                            "content_block_start" => {
                                let idx = ev["index"].as_u64().unwrap_or(0) as usize;
                                let cb = &ev["content_block"];
                                match cb["type"].as_str().unwrap_or("") {
                                    "text" => {
                                        current_text = String::new();
                                    }
                                    "tool_use" => {
                                        let id = cb["id"].as_str().unwrap_or("").to_string();
                                        let name = cb["name"].as_str().unwrap_or("").to_string();
                                        // Emit tool_start event to client
                                        let _ = tx
                                            .send(Ok(Event::default().data(
                                                json!({
                                                    "id": completion_id,
                                                    "object": "chat.completion.chunk",
                                                    "model": model_id,
                                                    "choices": [{
                                                        "index": 0,
                                                        "delta": {
                                                            "tool_call_start": {
                                                                "id": id,
                                                                "name": name
                                                            }
                                                        },
                                                        "finish_reason": null
                                                    }]
                                                })
                                                .to_string(),
                                            )))
                                            .await;
                                        tool_blocks.insert(idx, (id, name, String::new()));
                                    }
                                    _ => {}
                                }
                            }

                            "content_block_delta" => {
                                let idx = ev["index"].as_u64().unwrap_or(0) as usize;
                                let delta = &ev["delta"];
                                match delta["type"].as_str().unwrap_or("") {
                                    "text_delta" => {
                                        let text = delta["text"].as_str().unwrap_or("");
                                        current_text.push_str(text);
                                        full_assistant_text.push_str(text);
                                        let _ = tx
                                            .send(Ok(Event::default().data(
                                                json!({
                                                    "id": completion_id,
                                                    "object": "chat.completion.chunk",
                                                    "model": model_id,
                                                    "choices": [{
                                                        "index": 0,
                                                        "delta": {"content": text},
                                                        "finish_reason": null
                                                    }]
                                                })
                                                .to_string(),
                                            )))
                                            .await;
                                    }
                                    "input_json_delta" => {
                                        let partial = delta["partial_json"].as_str().unwrap_or("");
                                        if let Some(entry) = tool_blocks.get_mut(&idx) {
                                            entry.2.push_str(partial);
                                        }
                                    }
                                    _ => {}
                                }
                            }

                            "content_block_stop" => {
                                let idx = ev["index"].as_u64().unwrap_or(0) as usize;
                                if let Some((_, _, _)) = tool_blocks.get(&idx) {
                                    // tool_use block finalized — nothing extra to emit
                                } else if !current_text.is_empty() {
                                    assistant_content
                                        .push(json!({"type": "text", "text": current_text}));
                                    current_text = String::new();
                                }
                            }

                            "message_delta" => {
                                total_output += ev["usage"]["output_tokens"].as_u64().unwrap_or(0);
                                stop_reason = ev["delta"]["stop_reason"]
                                    .as_str()
                                    .unwrap_or("end_turn")
                                    .to_string();
                            }

                            _ => {}
                        }
                    }
                }
            }

            if stop_reason != "tool_use" {
                if full_assistant_text.is_empty() && tool_calls_made == 0 {
                    emit_stream_error(
                        &tx,
                        &completion_id,
                        &model_id,
                        "upstream stream ended without any assistant content",
                        thread_id.as_deref(),
                    )
                    .await;
                    break 'rounds;
                }

                // Final text chunk — persist and send [DONE]
                if let Some(ref tid) = thread_id {
                    let _ = thread_store
                        .append(
                            &tenant_id,
                            tid,
                            Message {
                                role: "assistant".into(),
                                content: full_assistant_text.clone(),
                                tool_calls: None,
                                timestamp: Utc::now(),
                                seq: 0,
                            },
                        )
                        .await;
                    maybe_set_title(&thread_store, &tenant_id, tid, &full_assistant_text).await;
                }

                // Index recent thread messages into the workspace node so the full
                // conversation history is searchable (not just the latest turn).
                if let (Some(node_id), Some(tid)) = (workspace_node_id, thread_id.as_ref()) {
                    let recent = thread_store
                        .messages(&tenant_id, tid)
                        .await
                        .unwrap_or_default();
                    // Take the last 30 messages (~15 turns) so the snippet stays within
                    // the 32 KB limit enforced by index_content.
                    let snippet: String = recent
                        .iter()
                        .rev()
                        .take(30)
                        .rev()
                        .map(|m| format!("{}: {}", m.role, m.content))
                        .collect::<Vec<_>>()
                        .join("\n\n");
                    let _ = state
                        .workspace_store
                        .index_content(&tenant_id, node_id, &snippet)
                        .await;
                }

                // Emit metrics for streaming completion.
                let model_label = [metrics::kv("model", model_id.as_str())];
                metrics::llm_requests().add(1, &model_label);
                metrics::llm_input_tokens().record(total_input, &model_label);
                metrics::llm_output_tokens().record(total_output, &model_label);

                // Final chunk with usage
                let _ = tx
                    .send(Ok(Event::default().data(
                        json!({
                            "id": completion_id,
                            "object": "chat.completion.chunk",
                            "model": model_id,
                            "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
                            "usage": {
                                "prompt_tokens": total_input,
                                "completion_tokens": total_output,
                                "total_tokens": total_input + total_output,
                            },
                            "tool_calls_made": tool_calls_made,
                            "thread_id": thread_id,
                        })
                        .to_string(),
                    )))
                    .await;
                let _ = tx.send(Ok(Event::default().data("[DONE]"))).await;
                break 'rounds;
            }

            // ── Execute tool calls ────────────────────────────────────────────
            // Build the assistant message content array
            if !current_text.is_empty() {
                assistant_content.push(json!({"type": "text", "text": current_text}));
            }
            let mut sorted_blocks: Vec<_> = tool_blocks.iter().collect();
            sorted_blocks.sort_by_key(|(idx, _)| **idx);
            for (_idx, (id, name, json_str)) in sorted_blocks {
                let parsed_input: Value = serde_json::from_str(json_str).unwrap_or(json!({}));
                assistant_content.push(json!({
                    "type": "tool_use",
                    "id": id,
                    "name": name,
                    "input": parsed_input,
                }));
            }
            messages.push(json!({"role": "assistant", "content": assistant_content}));

            let mut tool_results: Vec<Value> = vec![];
            let mut sorted_tools: Vec<_> = tool_blocks.drain().collect();
            sorted_tools.sort_by_key(|(idx, _)| *idx);

            for (_idx, (id, name, json_str)) in sorted_tools {
                let parsed_input: Value = serde_json::from_str(&json_str).unwrap_or(json!({}));

                info!(round, tool = name, "executing tool (stream)");
                tool_calls_made += 1;

                let result = match resolve_and_invoke(&state, &name, &parsed_input, &tenant).await {
                    Ok(v) => v.to_string(),
                    Err(e) => {
                        warn!(tool = name, error = %e, "tool invocation failed");
                        format!("Error: {e}")
                    }
                };

                // Emit tool_result event
                let _ = tx
                    .send(Ok(Event::default().data(
                        json!({
                            "id": completion_id,
                            "object": "chat.completion.chunk",
                            "model": model_id,
                            "choices": [{
                                "index": 0,
                                "delta": {
                                    "tool_call_result": {
                                        "tool_use_id": id,
                                        "name": name,
                                        "result": result,
                                    }
                                },
                                "finish_reason": null
                            }]
                        })
                        .to_string(),
                    )))
                    .await;

                tool_results.push(json!({
                    "type": "tool_result",
                    "tool_use_id": id,
                    "content": result,
                }));
            }

            messages.push(json!({"role": "user", "content": tool_results}));
        }
    });

    Sse::new(ReceiverStream::new(rx))
}

// ── Helpers ───────────────────────────────────────────────────────────────────

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

async fn emit_stream_error(
    tx: &mpsc::Sender<Result<Event, Infallible>>,
    completion_id: &str,
    model_id: &str,
    message: &str,
    thread_id: Option<&str>,
) {
    let text = format!("Error: {message}");
    let _ = tx
        .send(Ok(Event::default().data(
            json!({
                "id": completion_id,
                "object": "chat.completion.chunk",
                "model": model_id,
                "choices": [{
                    "index": 0,
                    "delta": {"content": text},
                    "finish_reason": null
                }],
                "thread_id": thread_id,
            })
            .to_string(),
        )))
        .await;

    let _ = tx
        .send(Ok(Event::default().data(
            json!({
                "id": completion_id,
                "object": "chat.completion.chunk",
                "model": model_id,
                "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
                "thread_id": thread_id,
            })
            .to_string(),
        )))
        .await;

    let _ = tx.send(Ok(Event::default().data("[DONE]"))).await;
}

async fn resolve_and_invoke(
    state: &Arc<AppState>,
    full_tool_name: &str,
    input: &Value,
    tenant: &ResolvedTenant,
) -> anyhow::Result<Value> {
    let (cap_name, tool_name) = full_tool_name
        .split_once("__")
        .ok_or_else(|| anyhow::anyhow!("invalid tool name: {full_tool_name}"))?;

    // Grab the provider Arc under a short-lived lock so we don't hold it across await.
    let provider = {
        let registry = state.registry.lock().unwrap();
        registry
            .get_provider(cap_name)
            .ok_or_else(|| anyhow::anyhow!("no provider registered for '{cap_name}'"))?
    };

    provider.invoke(tool_name, input, Some(&tenant.0)).await
}

fn err500(msg: String) -> HttpError {
    map_rig_error(msg)
}
