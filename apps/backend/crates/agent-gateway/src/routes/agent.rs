/// POST /v1/agent/completions — Anthropic tool-use agent loop with optional thread memory.
///
/// Supports both blocking (default) and streaming (`"stream": true`) modes.
/// When streaming, emits OpenAI-compatible SSE chunks plus extra `tool_call_start` /
/// `tool_call_result` event types so clients can follow tool execution in real-time.
///
/// Pass `"thread_id": "<ulid>"` to load history from Postgres and persist the turn.
/// Pass `"max_turns": N` to override the default tool-call rounds (capped by plan tier).
use crate::mw::meter::AgentTurnStats;
use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use billing_core::events::{ActionType, UsageEvent};
use agent_core::{ContextBuilder, PlanLimits, map_rig_error};
use axum::{
    Extension, Json,
    extract::State,
    response::{
        IntoResponse, Response,
        sse::{Event, Sse},
    },
};
use chrono::Utc;
use common::audit::AuditEvent;
use std::time::Instant;
use common::error::HttpError;
use common::memory::thread::Message;
use common::metrics;
use futures::StreamExt;
use serde_json::{Value, json};
use std::collections::HashSet;
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
    Extension(limits): Extension<PlanLimits>,
    Json(req): Json<crate::routes::chat::ChatRequest>,
) -> Response {
    if !state
        .rate_limiter
        .check(&tenant.0.tenant_id, limits.rate_limit_rpm)
    {
        warn!("rate limit hit");
        return HttpError::rate_limit(None).into_response();
    }

    if req.stream.unwrap_or(false) {
        stream_agent(state, tenant, limits, req).await.into_response()
    } else {
        match blocking_agent(state, tenant, limits, req).await {
            Ok((v, stats)) => {
                let mut resp = Json(v).into_response();
                resp.extensions_mut().insert(stats);
                resp
            }
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
    /// True when `thread_id` is Some and the loaded history is empty — i.e. this
    /// turn either created a new thread (via `resolve_for_node`) or is the first
    /// turn after creation. Used to trigger a `threads` invalidation SSE delta
    /// at end-of-turn so the recents list refreshes (PR 3.A.6).
    thread_was_new: bool,
    tenant_id: String,
    tools: Vec<Value>,
    messages: Vec<Value>,
    effective_system: Option<String>,
    /// Parsed workspace node ULID, used to index chat content for search.
    workspace_node_id: Option<_Ulid>,
    max_invokes_per_turn: usize,
    /// Routing metadata emitted as the first SSE delta in streaming turns (PR 3.B).
    routing_meta: Value,
}

async fn build_ctx(
    state: &Arc<AppState>,
    tenant: &ResolvedTenant,
    limits: PlanLimits,
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
    // Prefer PlanCatalog limits (runtime-configurable) over compiled-in tier defaults.
    let catalog = state.plan_catalog.by_tier(&tenant.0.plan);
    let catalog_max_tokens = catalog.map(|p| p.max_tokens).unwrap_or(limits.max_tokens);
    let catalog_max_turns = catalog
        .and_then(|p| p.max_turns_per_day)
        .unwrap_or(limits.max_turns as u64) as usize;

    let max_tokens = req.max_tokens.unwrap_or(catalog_max_tokens).min(catalog_max_tokens);

    Span::current().record("gen_ai.request.model", model_id.as_str());

    let tenant_id = tenant.0.tenant_id.to_string();
    let thread_store = Arc::clone(&state.thread_store);

    // Effective max_rounds: honour request value but cap at plan limit.
    let max_rounds = req
        .max_turns
        .map(|s| (s as usize).min(catalog_max_turns))
        .unwrap_or(catalog_max_turns);

    // Resolve effective thread_id via ConversationService:
    //   1. Explicit `thread_id` on the request always wins.
    //   2. If `workspace_node_id` is provided, resolve (or lazily create) the
    //      thread bound to that node via ConversationService.
    //   3. Otherwise create a floating thread so multi-turn conversations
    //      maintain history even without a workspace node (e.g. quick-reply
    //      chips like "Confirm" retain the prior turn's context).
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
        // Case 3: No workspace context — create a floating thread so multi-turn
        // conversations maintain history without requiring a workspace node.
        match state.conversation_service.create(&tenant.0, None).await {
            Ok(tid) => Some(tid.to_string()),
            Err(e) => {
                warn!(error = %e, "failed to create floating thread for context-free turn");
                None
            }
        }
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

    // Recents-list refresh signal (PR 3.A.6). A thread is "new" when it exists
    // for this turn but has no prior messages — either freshly minted by
    // `resolve_for_node` or about to be its first turn. We use this to broadcast
    // a `threads` invalidation event after the turn so client recents refresh.
    let thread_was_new = thread_id.is_some() && history.is_empty();

    // ── Capability routing pipeline (PR 2.A + 2.B) ───────────────────────────
    //
    // Merge order (load-bearing):
    //   1. Semantic router — ANN top-K + include_always.
    //   2. Lexical prefilter — word-boundary keyword matches from manifests (PR 2.B.3).
    //   3. forced_capability pin — prepend + dedup; never truncated (PR 2.A.3).
    //   4. Confidence threshold check — bump counter if max_score < min_confidence (2.A.3.1).
    //   5. Truncate to max_tools_per_turn.
    let user_query = req
        .messages
        .iter()
        .rfind(|m| m.role == "user")
        .map(|m| m.content.as_str())
        .unwrap_or("");
    let router_total_start = Instant::now();

    // Stage 1: Semantic routing (ANN + include_always).
    let (mut tools, max_score) = {
        let semantic_span = tracing::info_span!(
            "router.semantic",
            tools_returned = tracing::field::Empty,
            max_score = tracing::field::Empty,
        );
        let _enter = semantic_span.enter();
        let stage_start = Instant::now();
        let result = state
            .semantic_router
            .tool_definitions_and_score(user_query, Some(&tenant.0))
            .await;
        let stage_ms = stage_start.elapsed().as_secs_f64() * 1000.0;
        if let Some(rm) = state.router_metrics.as_ref() {
            rm.observe_stage_ms("semantic", stage_ms);
        }
        match result {
            Ok((defs, score)) => {
                Span::current().record("tools_returned", defs.len());
                Span::current().record("max_score", score);
                (defs, score)
            }
            Err(e) => {
                warn!(error = %e, "semantic router failed — continuing with zero tools for this turn");
                (vec![], 0.0_f64)
            }
        }
    };

    // Stage 2: Lexical prefilter (PR 2.B.3) — word-boundary keyword matching.
    let lexical_hits: Vec<String> = {
        let stage_start = Instant::now();
        let hits = {
            let registry = state.registry.lock().unwrap();
            registry.lexical_hint_capabilities(user_query, &tenant_id)
        };
        let stage_ms = stage_start.elapsed().as_secs_f64() * 1000.0;
        if let Some(rm) = state.router_metrics.as_ref() {
            rm.observe_stage_ms("lexical", stage_ms);
        }
        hits
    };
    // Collect tool defs from lexical-only capabilities (those not already in semantic results).
    let semantic_cap_names: HashSet<String> = tools
        .iter()
        .filter_map(|v| {
            v.get("name")?.as_str().and_then(|n| {
                n.split_once("__").map(|(cap, _)| cap.to_string())
            })
        })
        .collect();
    // Gather lexical-only tool defs; prepend them so they survive max_tools truncation
    // (PR 2.B.3 fix: appending caused keyword-matched caps to be silently dropped when
    // the semantic result already filled the tool budget).
    let mut lex_only: Vec<Value> = Vec::new();
    for lex_cap in &lexical_hits {
        if !semantic_cap_names.contains(lex_cap.as_str()) {
            let cap_defs: Vec<Value> = {
                let registry = state.registry.lock().unwrap();
                registry
                    .tools_for_capability_exact_for_tenant(lex_cap, &tenant_id)
                    .unwrap_or_default()
            };
            lex_only.extend(cap_defs);
        }
    }
    if !lex_only.is_empty() {
        // Prepend lexical tools before semantic results; deduplicate by tool name.
        let lex_names: HashSet<String> = lex_only
            .iter()
            .filter_map(|v| v.get("name")?.as_str().map(|s| s.to_string()))
            .collect();
        let semantic_remainder: Vec<Value> = tools
            .into_iter()
            .filter(|t| {
                t.get("name")
                    .and_then(|n| n.as_str())
                    .map(|n| !lex_names.contains(n))
                    .unwrap_or(true)
            })
            .collect();
        tools = lex_only;
        tools.extend(semantic_remainder);
    }

    // Stage 3: forced_capability pin (PR 2.A.3).
    // Prepend tools from the pinned capability BEFORE truncation so they survive.
    // Security: server-side tenant-allowlist validation — never trust client value.
    let max_tools = state.router_quota.max_tools_per_turn.max(1);
    let forced_cap_outcome: &'static str = if let Some(ref cap_name) = req.forced_capability {
        let forced_span = tracing::info_span!("router.forced_pin", cap = cap_name.as_str());
        let _enter = forced_span.enter();
        let stage_start = Instant::now();
        let pinned_tools: Option<Vec<Value>> = {
            let registry = state.registry.lock().unwrap();
            registry.tools_for_capability_exact_for_tenant(cap_name, &tenant_id)
        };
        let stage_ms = stage_start.elapsed().as_secs_f64() * 1000.0;
        if let Some(rm) = state.router_metrics.as_ref() {
            rm.observe_stage_ms("forced_pin", stage_ms);
        }
        match pinned_tools {
            Some(pinned) if !pinned.is_empty() => {
                tools = merge_pinned(pinned, tools, max_tools);
                "pinned_kept"
            }
            Some(_) => {
                warn!(
                    cap = cap_name,
                    "forced_capability resolved but has no tools — pin dropped"
                );
                if tools.len() > max_tools { tools.truncate(max_tools); }
                "dropped"
            }
            None => {
                // Capability unknown or not in this tenant's scope — ignore silently (security).
                warn!(
                    cap = cap_name,
                    tenant = tenant_id,
                    "forced_capability not found or disabled for tenant — ignoring"
                );
                if tools.len() > max_tools { tools.truncate(max_tools); }
                "dropped"
            }
        }
    } else {
        // No forced pin — apply normal truncation.
        if tools.len() > max_tools { tools.truncate(max_tools); }
        "none"
    };

    // Stage 4: Confidence threshold check (PR 2.A.3.1).
    let low_confidence = max_score < state.router_quota.min_confidence;
    if low_confidence {
        if let Some(rm) = state.router_metrics.as_ref() {
            rm.record_low_confidence_turn();
        }
        warn!(
            max_score,
            threshold = state.router_quota.min_confidence,
            "router: low-confidence turn (max_score < threshold)"
        );
    }

    // Stage 5: Merge-span metrics.
    if let Some(rm) = state.router_metrics.as_ref() {
        let total_ms = router_total_start.elapsed().as_secs_f64() * 1000.0;
        rm.observe_stage_ms("merge", total_ms);
        rm.observe_stage_ms("total", total_ms);
        rm.observe_tools_per_turn(tools.len());
        rm.record_forced_capability(forced_cap_outcome);
    }

    // Audit routing decision for observability and compliance (PR 2.A.4).
    let selected_capabilities: Vec<String> = {
        let mut out: Vec<String> = Vec::new();
        for t in &tools {
            if let Some(name) = t.get("name").and_then(|v| v.as_str()) {
                let cap = name.split_once("__").map(|(c, _)| c).unwrap_or(name).to_string();
                if !out.contains(&cap) { out.push(cap); }
            }
        }
        out
    };
    let pinned_tools: Vec<String> = if forced_cap_outcome == "pinned_kept" {
        if let Some(ref cap_name) = req.forced_capability {
            let registry = state.registry.lock().unwrap();
            registry
                .tools_for_capability_exact_for_tenant(cap_name, &tenant_id)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|v| v.get("name").and_then(|n| n.as_str()).map(|s| s.to_string()))
                .collect()
        } else {
            vec![]
        }
    } else {
        vec![]
    };
    let _ = state
        .audit_store
        .append(
            AuditEvent::new(tenant_id.clone(), "semantic_router.select").with_metadata(json!({
                "selected_top_k": tools.len(),
                "selected_capabilities": selected_capabilities,
                "forced_capability": req.forced_capability,
                "forced_cap_outcome": forced_cap_outcome,
                "pinned_tool_count": pinned_tools.len(),
                "pinned_tools": pinned_tools,
                "lexical_hits": lexical_hits,
                "max_score": max_score,
                "threshold_met": !low_confidence,
                "low_confidence": low_confidence,
            })),
        )
        .await;

    let non_system: Vec<_> = req.messages.iter().filter(|m| m.role != "system").collect();

    // Index of the last user message among non-system messages (for attachment injection).
    let last_user_pos = non_system.iter().rposition(|m| m.role == "user");

    let new_messages: Vec<Value> = non_system
        .iter()
        .enumerate()
        .map(|(i, m)| {
            // Inject attachment content blocks into the last user turn only.
            if m.role == "user"
                && Some(i) == last_user_pos
                && !req.attachment_content.is_empty()
            {
                let mut content: Vec<Value> = req.attachment_content.clone();
                if !m.content.is_empty() {
                    content.push(json!({"type": "text", "text": m.content}));
                }
                json!({"role": "user", "content": content})
            } else {
                json!({"role": m.role, "content": m.content})
            }
        })
        .collect();

    // Default tool-use guard: merged into any caller-provided system prompt.
    // Prevents Claude from calling tools for unrelated or ambiguous queries.
    const TOOL_GUARD: &str = "Only call tools when the user's request explicitly requires them. \
        For general conversation, questions, or anything that can be answered directly, \
        respond without invoking tools.";

    let system_content = req
        .messages
        .iter()
        .find(|m| m.role == "system")
        .map(|m| m.content.clone());

    // Inject the tool guard unless the caller already provides a system prompt
    // (caller-provided prompts are assumed to already encode the correct behaviour).
    let system_content = Some(match system_content {
        Some(existing) => format!("{existing}\n\n{TOOL_GUARD}"),
        None => TOOL_GUARD.to_string(),
    });

    let base_system = if let Some(ref tid) = thread_id {
        let summary = thread_store
            .get(&tenant_id, tid)
            .await
            .ok()
            .flatten()
            .and_then(|t| t.summary);
        match (system_content, summary) {
            (Some(sys), Some(sum)) => Some(format!("{sys}\n\n[Conversation summary: {sum}]")),
            (sys, _) => sys,
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

    let routing_meta = json!({
        "forced_capability": req.forced_capability,
        "selected_capabilities": selected_capabilities,
        "pinned_tools": pinned_tools,
        "lexical_hits": lexical_hits,
        "max_score": max_score,
    });

    Ok(AgentCtx {
        api_key,
        model_id,
        max_tokens,
        max_rounds,
        thread_id,
        thread_was_new,
        tenant_id,
        tools,
        messages: history,
        effective_system,
        workspace_node_id,
        max_invokes_per_turn: state.router_quota.max_invokes_per_turn.max(1),
        routing_meta,
    })
}

// ── Blocking path ───────────────────────────────────────────────────

async fn blocking_agent(
    state: Arc<AppState>,
    tenant: ResolvedTenant,
    limits: PlanLimits,
    req: crate::routes::chat::ChatRequest,
) -> Result<(Value, AgentTurnStats), HttpError> {
    let start = Instant::now();
    let ctx = build_ctx(&state, &tenant, limits, &req).await?;

    let AgentCtx {
        api_key,
        model_id,
        max_tokens,
        max_rounds,
        thread_id,
        thread_was_new: _,
        tenant_id,
        tools,
        mut messages,
        effective_system,
        workspace_node_id,
        max_invokes_per_turn,
        routing_meta: _,
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
                // Blocking path: title may be set but we do not broadcast threads
                // invalidation (no SSE channel to emit on; bus subscribers are
                // not connected for the blocking turn).
                let _ = maybe_set_title(&thread_store, &tenant_id, tid, &text).await;
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
                let tid_owned = tenant_id.clone();
                let node_id_str = node_id.to_string();
                let emb_svc = Arc::clone(&state.embedding_service);
                let vs = Arc::clone(&state.vector_store);
                tokio::spawn(async move {
                    const CHUNK: usize = 1500;
                    let chunks: Vec<String> = snippet
                        .chars()
                        .collect::<Vec<_>>()
                        .chunks(CHUNK)
                        .map(|c| c.iter().collect::<String>())
                        .collect();
                    if let Ok(embeddings) = emb_svc.embed_documents(chunks.clone()).await {
                        for (i, (chunk, emb)) in chunks.iter().zip(embeddings.iter()).enumerate() {
                            let chunk_id = format!("{node_id_str}_t{i}");
                            let _ = vs
                                .upsert_content_embedding_full(
                                    &chunk_id,
                                    &node_id_str,
                                    i as i32,
                                    chunk,
                                    emb,
                                    &tid_owned,
                                    "",
                                    &[],
                                )
                                .await;
                        }
                    }
                });
            }

            let stats = AgentTurnStats {
                tokens: total_input + total_output,
                turns: 1,
                model: model_id.clone(),
                duration_ms: start.elapsed().as_millis() as u64,
            };
            return Ok((json!({
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
            }), stats));
        }

        // Tool use round
        messages.push(json!({"role": "assistant", "content": content}));
        let mut tool_results: Vec<Value> = vec![];
        let mut invoke_limit_hit = false;

        for block in &content {
            if block["type"] != "tool_use" {
                continue;
            }
            let call_id = block["id"].as_str().unwrap_or("").to_string();
            let tool_name = block["name"].as_str().unwrap_or("");

            if invoke_limit_hit || tool_calls_made >= max_invokes_per_turn {
                // Soft limit: inject synthetic result so the LLM can summarise
                // partial completion and invite the user to say "continue".
                invoke_limit_hit = true;
                tool_results.push(json!({
                    "type": "tool_result",
                    "tool_use_id": call_id,
                    "content": format!(
                        "Skipped: per-turn tool limit ({max_invokes_per_turn}) reached. \
                         Summarise what was completed so far and ask the user to reply \
                         'continue' to process the remaining items."
                    ),
                    "is_error": true,
                }));
                continue;
            }

            let tool_input = &block["input"];
            info!(round, tool = tool_name, "executing tool");
            tool_calls_made += 1;

            let result = match resolve_and_invoke(&state, tool_name, tool_input, &tenant).await {
                Ok((v, _paths)) => {
                    // Note: the blocking (non-SSE) path doesn't emit SSE deltas, so
                    // workspace invalidation for blocking calls is handled by the
                    // caller's own response mechanism. Paths are intentionally not
                    // accumulated here.
                    v.to_string()
                }
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
    limits: PlanLimits,
    req: crate::routes::chat::ChatRequest,
) -> Sse<ReceiverStream<Result<Event, Infallible>>> {
    let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(128);

    tokio::spawn(async move {
        let ctx = match build_ctx(&state, &tenant, limits, &req).await {
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
            thread_was_new,
            tenant_id,
            tools,
            mut messages,
            effective_system,
            workspace_node_id,
            max_invokes_per_turn,
            routing_meta,
        } = ctx;

        let thread_store = Arc::clone(&state.thread_store);
        let http = reqwest::Client::new();
        let completion_id = format!("chatcmpl-{}", Uuid::new_v4());
        let mut tool_calls_made = 0usize;
        let mut total_input = 0u64;
        let mut total_output = 0u64;
        let mut full_assistant_text = String::new();
        // Collect virtual paths written by ArtifactBridge across all tool calls (PR 3.A).
        let mut all_changed_paths: Vec<String> = vec![];
        // Whether maybe_set_title actually set a title on this turn (PR 3.A.6).
        let mut title_was_set = false;

        // Emit routing_meta as the very first SSE delta so the client can
        // display the capability hint chip immediately (PR 3.B).
        let _ = tx
            .send(Ok(Event::default().data(
                json!({
                    "id": completion_id,
                    "object": "chat.completion.chunk",
                    "model": model_id,
                    "choices": [{
                        "index": 0,
                        "delta": { "routing_meta": routing_meta },
                        "finish_reason": null
                    }]
                })
                .to_string(),
            )))
            .await;

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
                    // Track whether a new title was set so the end-of-turn block
                    // can emit a `threads` resource_invalidated SSE delta + bus
                    // broadcast (PR 3.A.6).
                    title_was_set =
                        maybe_set_title(&thread_store, &tenant_id, tid, &full_assistant_text).await;
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
                    let tid_owned = tenant_id.clone();
                    let node_id_str = node_id.to_string();
                    let emb_svc = Arc::clone(&state.embedding_service);
                    let vs = Arc::clone(&state.vector_store);
                    tokio::spawn(async move {
                        const CHUNK: usize = 1500;
                        let chunks: Vec<String> = snippet
                            .chars()
                            .collect::<Vec<_>>()
                            .chunks(CHUNK)
                            .map(|c| c.iter().collect::<String>())
                            .collect();
                        if let Ok(embeddings) = emb_svc.embed_documents(chunks.clone()).await {
                            for (i, (chunk, emb)) in chunks.iter().zip(embeddings.iter()).enumerate() {
                                let chunk_id = format!("{node_id_str}_t{i}");
                                let _ = vs
                                    .upsert_content_embedding_full(
                                        &chunk_id,
                                        &node_id_str,
                                        i as i32,
                                        chunk,
                                        emb,
                                        &tid_owned,
                                        "",
                                        &[],
                                    )
                                    .await;
                            }
                        }
                    });
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
                // Emit resource_invalidated delta if any workspace paths were mutated (PR 3.A).
                if !all_changed_paths.is_empty() {
                    // Dedup while preserving order.
                    let mut seen = std::collections::HashSet::new();
                    let deduped: Vec<&str> = all_changed_paths
                        .iter()
                        .filter(|p| seen.insert(p.as_str()))
                        .map(|p| p.as_str())
                        .collect();
                    let _ = tx
                        .send(Ok(Event::default().data(
                            json!({
                                "id": completion_id,
                                "object": "chat.completion.chunk",
                                "model": model_id,
                                "choices": [{
                                    "index": 0,
                                    "delta": {
                                        "resource_invalidated": {
                                            "resource": "workspace",
                                            "scope": tenant_id,
                                            "changed_keys": deduped,
                                        }
                                    },
                                    "finish_reason": null
                                }]
                            })
                            .to_string(),
                        )))
                        .await;

                    // Also broadcast to the InvalidationBus for any out-of-band subscribers.
                    let _ = state.invalidation_bus.send(
                        agent_core::realtime::invalidation::InvalidationEvent::new("workspace", &tenant_id)
                            .with_keys(all_changed_paths.clone()),
                    );
                }

                // Emit threads invalidation if the turn either created a new thread
                // or set a title for an existing one (PR 3.A.6). Single delta per
                // turn — coalesces both signals so the recents list only re-fetches
                // when it's actually likely to look different.
                if (thread_was_new || title_was_set)
                    && let Some(ref tid) = thread_id
                {
                    let changed_keys = vec![tid.as_str()];
                    let _ = tx
                        .send(Ok(Event::default().data(
                            json!({
                                "id": completion_id,
                                "object": "chat.completion.chunk",
                                "model": model_id,
                                "choices": [{
                                    "index": 0,
                                    "delta": {
                                        "resource_invalidated": {
                                            "resource": "threads",
                                            "scope": tenant_id,
                                            "changed_keys": changed_keys,
                                        }
                                    },
                                    "finish_reason": null
                                }]
                            })
                            .to_string(),
                        )))
                        .await;
                    let _ = state.invalidation_bus.send(
                        agent_core::realtime::invalidation::InvalidationEvent::new("threads", &tenant_id)
                            .with_keys(vec![tid.clone()]),
                    );
                }

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
            // Track whether we've hit the per-turn invocation cap so that
            // remaining tools get synthetic error results and the LLM can
            // summarise partial completion and invite "continue".
            let mut invoke_limit_hit = false;

            for (_idx, (id, name, json_str)) in sorted_tools {
                let parsed_input: Value = serde_json::from_str(&json_str).unwrap_or(json!({}));

                if invoke_limit_hit || tool_calls_made >= max_invokes_per_turn {
                    // Soft limit: inject a synthetic tool_result so Anthropic
                    // receives a complete assistant→user exchange, then let the
                    // LLM produce a summary + "reply 'continue' to proceed" message.
                    invoke_limit_hit = true;
                    tool_results.push(json!({
                        "type": "tool_result",
                        "tool_use_id": id,
                        "content": format!(
                            "Skipped: per-turn tool limit ({max_invokes_per_turn}) reached. \
                             Summarise what was completed so far and ask the user to reply \
                             'continue' to process the remaining items."
                        ),
                        "is_error": true,
                    }));
                    continue;
                }

                info!(round, tool = name, "executing tool (stream)");
                tool_calls_made += 1;

                let result = match resolve_and_invoke(&state, &name, &parsed_input, &tenant).await {
                    Ok((v, paths)) => {
                        all_changed_paths.extend(paths);
                        v.to_string()
                    }
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

        // Stream complete — report usage directly (can't inject into SSE response extensions).
        if let (Some(billing), Some(quota)) = (&state.billing, &state.quota) {
            let tid = tenant.0.tenant_id.to_string();
            let turn_event = UsageEvent::new(tid.clone(), tid.clone(), ActionType::AgentTurn, 1);
            if let Err(e) = billing.report_usage(turn_event).await {
                warn!(error = %e, "stream metering: report_usage(agent_turn) failed");
            }
            quota.record(&tid, &ActionType::AgentTurn, 1).await;
            if total_input + total_output > 0 {
                let tok_event = UsageEvent::new(tid.clone(), tid.clone(), ActionType::Token, total_input + total_output);
                if let Err(e) = billing.report_usage(tok_event).await {
                    warn!(error = %e, "stream metering: report_usage(token) failed");
                }
                quota.record(&tid, &ActionType::Token, total_input + total_output).await;
            }
        }
    });

    Sse::new(ReceiverStream::new(rx))
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Set an auto-generated title for the thread if one is not yet set.
/// Returns `true` if `set_title` was actually invoked (so the caller can emit
/// a `threads` invalidation event). PR 3.A.6.
async fn maybe_set_title(
    store: &Arc<dyn common::memory::ThreadStore>,
    tenant_id: &str,
    thread_id: &str,
    assistant_text: &str,
) -> bool {
    let already_titled = store
        .get(tenant_id, thread_id)
        .await
        .ok()
        .flatten()
        .map(|t| t.title.is_some())
        .unwrap_or(false);

    if !already_titled && !assistant_text.is_empty() {
        let title: String = assistant_text.chars().take(60).collect();
        return store.set_title(tenant_id, thread_id, title).await.is_ok();
    }
    false
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

/// PR 2.D — read-before-write: if the tool's manifest declares
/// `read_before_write = "<field>"`, read the file at `input[field]` from the
/// `WorkspaceContentStore` and inject `_current_content` (and `_is_new_file`)
/// into the input before the tool runs. Returns `Some(patched_input)` when
/// injection happened, `None` when not applicable.
async fn maybe_inject_current_content(
    state: &Arc<AppState>,
    full_tool_name: &str,
    input: &Value,
    tenant_id: &str,
) -> Option<Value> {
    // Split "code-project__add_dependency" → ("code-project", "add_dependency").
    let (safe_cap, tool_name) = full_tool_name.split_once("__")?;

    // The registry key might use dots ("code.project") while the tool name uses
    // underscores for dots; try both forms.  Keep the lock guard in a nested
    // block so it is definitely dropped before the async workspace read below.
    let (_field, path) = {
        let cap_name_dot = safe_cap.replace('_', ".");
        let registry = state.registry.lock().ok()?;
        let card = registry
            .get(safe_cap)
            .or_else(|| registry.get(&cap_name_dot))
            .cloned()?;
        // registry guard dropped here — before any .await
        let tool_def = card.manifest.tools.iter().find(|t| t.name == tool_name)?;
        let field = tool_def.read_before_write.clone()?;
        let path = input.get(&field)?.as_str()?.to_owned();
        if path.is_empty() {
            return None;
        }
        (field, path)
    };

    let mut patched = input.clone();
    match state.workspace_content.read(tenant_id, &path).await {
        Ok(content) => {
            patched["_current_content"] = serde_json::json!(content);
            patched["_is_new_file"] = serde_json::json!(false);
            tracing::debug!(
                tool = full_tool_name,
                path,
                "read_before_write: injected _current_content ({} bytes)",
                content.len()
            );
        }
        Err(_) => {
            // File doesn't exist — new-file branch.
            patched["_current_content"] = serde_json::Value::Null;
            patched["_is_new_file"] = serde_json::json!(true);
            tracing::debug!(
                tool = full_tool_name,
                path,
                "read_before_write: file not found, injecting _is_new_file=true"
            );
        }
    }
    Some(patched)
}

/// Returns `(tool_output_value, changed_virtual_paths)`.
/// `changed_virtual_paths` is non-empty when the tool materialised workspace artifacts (PR 3.A).
async fn resolve_and_invoke(
    state: &Arc<AppState>,
    full_tool_name: &str,
    input: &Value,
    tenant: &ResolvedTenant,
) -> anyhow::Result<(Value, Vec<String>)> {
    // PR 2.D — inject current file content before chain runs (prevents fabrication).
    let injected = maybe_inject_current_content(state, full_tool_name, input, &tenant.0.tenant_id).await;
    let effective_input = injected.as_ref().unwrap_or(input);

    let mut raw_result = state
        .semantic_router
        .invoke(full_tool_name, effective_input, Some(&tenant.0))
        .await?;

    // Phase 4 — ArtifactBridge: materialise any file artifacts into RustFS + workspace.
    // Phase 9 — if static hosting, attach public_url to raw_result so the agent loop
    //            streams it as a clickable card.
    let mut changed_paths: Vec<String> = vec![];

    if let Some(ref bridge) = state.artifact_bridge
        && let Ok(tool_out) =
            serde_json::from_value::<common::artifact::ToolOutput>(raw_result.clone())
        && !tool_out.artifacts.is_empty()
    {
        let tool_short = full_tool_name.split("__").next().unwrap_or(full_tool_name);
        if let Ok((public_url, paths)) = bridge
            .process_if_artifacts(
                &tenant.0.tenant_id,
                tenant.0.user_id.as_deref(),
                tool_short,
                None,
                &tool_out,
            )
            .await
        {
            changed_paths = paths;
            if let Some(url) = public_url {
                // Merge public_url into the result so the LLM and UI see it.
                if let Some(obj) = raw_result.as_object_mut() {
                    obj.insert("public_url".to_string(), serde_json::json!(url));
                }
            }
        }
    }

    // Native storage-workspace tools mutate the workspace tree but don't go through
    // the ArtifactBridge, so changed_paths would otherwise stay empty and no
    // `resource_invalidated` SSE event would be emitted. Detect those mutations here
    // and push a sentinel so the end-of-turn broadcast always fires.
    //
    // Tool names are formed as `<capability-name>__<tool-name>` where the capability
    // name comes from capability.toml `name` (not the namespace). For storage-workspace
    // this is "storage-workspace" (hyphen), giving e.g. "storage-workspace__create_folder".
    //
    // Read-only tools (list_folders, show_tree, find_by_name) are excluded — they
    // don't change any state so they must not trigger a spurious re-fetch.
    const STORAGE_WS_PREFIX: &str = "storage-workspace__";
    const STORAGE_WS_READONLY: &[&str] = &[
        "storage-workspace__list_folders",
        "storage-workspace__show_tree",
        "storage-workspace__find_by_name",
    ];
    if changed_paths.is_empty()
        && full_tool_name.starts_with(STORAGE_WS_PREFIX)
        && !STORAGE_WS_READONLY.contains(&full_tool_name)
    {
        // "*" acts as a sentinel: the SSE handler only checks resource=="workspace",
        // so the exact key value doesn't matter to the frontend invalidation logic.
        changed_paths.push("*".to_string());
    }

    Ok((raw_result, changed_paths))
}

fn err500(msg: String) -> HttpError {
    map_rig_error(msg)
}

// ── Routing helpers ───────────────────────────────────────────────────────────

/// Merge `pinned` tool definitions before `semantic` ones (PR 2.A.3).
///
/// Algorithm:
/// 1. Start with `pinned` (these are always at the front).
/// 2. Append semantic tools that aren't already in `pinned` (dedup by `name`).
/// 3. Truncate to `max_tools` — pinned tools are at the front and survive.
///
/// This guarantees the forced capability's tools are position-0 and therefore
/// never evicted by the budget cap, even when the semantic router would not have
/// chosen them (e.g. cosine distance too high for the current query).
pub fn merge_pinned(pinned: Vec<Value>, semantic: Vec<Value>, max_tools: usize) -> Vec<Value> {
    let seen: HashSet<String> = pinned
        .iter()
        .filter_map(|v| v.get("name")?.as_str().map(|s| s.to_string()))
        .collect();

    let mut result = pinned;
    for tool in semantic {
        let name = tool.get("name").and_then(|n| n.as_str()).map(|s| s.to_string());
        if name.as_deref().map(|n| !seen.contains(n)).unwrap_or(true) {
            result.push(tool);
        }
    }
    result.truncate(max_tools);
    result
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(name: &str) -> Value {
        serde_json::json!({ "name": name, "description": "test" })
    }

    // 2.A.9 — basic forced_capability inclusion: pinned tool appears in output.
    #[test]
    fn merge_pinned_includes_forced_tool() {
        let pinned = vec![tool("extract_invoice")];
        let semantic = vec![tool("save_document"), tool("read_file")];
        let result = merge_pinned(pinned, semantic, 10);

        let names: Vec<&str> = result
            .iter()
            .filter_map(|v| v.get("name")?.as_str())
            .collect();

        assert!(
            names.contains(&"extract_invoice"),
            "pinned tool must appear in output; got {names:?}"
        );
        assert!(names.contains(&"save_document"));
        assert!(names.contains(&"read_file"));
    }

    // 2.A.10 — pinning guarantee: pinned tools survive truncation.
    //
    // Scenario: max_tools = 2, semantic has 3 tools, pinned has 1 tool that
    // would not be in the top-2 semantic results. After merge, the pinned tool
    // must be at position 0 and survive the truncation.
    #[test]
    fn merge_pinned_survives_truncation() {
        let pinned = vec![tool("extract_invoice")]; // would be evicted at max_tools=2
        let semantic = vec![tool("save_document"), tool("read_file"), tool("upload_file")];
        let result = merge_pinned(pinned, semantic, 2);

        assert_eq!(result.len(), 2, "result should be truncated to max_tools=2");
        assert_eq!(
            result[0].get("name").and_then(|v| v.as_str()),
            Some("extract_invoice"),
            "pinned tool must be at position 0 after truncation"
        );
        assert_eq!(
            result[1].get("name").and_then(|v| v.as_str()),
            Some("save_document"),
        );
    }

    // Dedup: if the pinned tool also appears in semantic, it must not be duplicated.
    #[test]
    fn merge_pinned_deduplicates_overlap() {
        let pinned = vec![tool("save_document")];
        let semantic = vec![tool("save_document"), tool("read_file")];
        let result = merge_pinned(pinned, semantic, 10);

        let names: Vec<&str> = result
            .iter()
            .filter_map(|v| v.get("name")?.as_str())
            .collect();
        let deduped_count = names.iter().filter(|&&n| n == "save_document").count();
        assert_eq!(deduped_count, 1, "save_document should appear exactly once; got {names:?}");
    }

    // Empty pinned list: output is just semantic (truncated).
    #[test]
    fn merge_pinned_empty_pinned_returns_semantic() {
        let semantic = vec![tool("a"), tool("b"), tool("c")];
        let result = merge_pinned(vec![], semantic.clone(), 2);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].get("name").and_then(|v| v.as_str()), Some("a"));
    }

    // max_tools = 0 returns empty regardless.
    #[test]
    fn merge_pinned_zero_max_tools_returns_empty() {
        let pinned = vec![tool("forced")];
        let semantic = vec![tool("a")];
        let result = merge_pinned(pinned, semantic, 0);
        assert!(result.is_empty(), "max_tools=0 must produce empty list");
    }
}
