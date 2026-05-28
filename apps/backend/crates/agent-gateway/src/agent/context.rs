//! `AgentCtx` — resolved per-request context — and `build_ctx` constructor.
//!
//! Step 2.3, 2.9: moved from `routes/agent.rs`; updated for parking_lot
//! registry reads and cross-tenant ownership checks.

use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use agent_core::{
    AgentMessage, ContentBlock, ContextBuilder, MessageContent, MessageRole, ModelError,
    PlanLimits, ToolRoutingDecision, estimate_input_tokens, map_rig_error,
    token_estimate_exceeds_limit,
};
use chrono::Utc;
use common::audit::AuditEvent;
use common::error::HttpError;
use common::memory::thread::Message;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;
use tracing::{Span, warn};
use ulid::Ulid as _Ulid;

// ── AgentCtx ──────────────────────────────────────────────────────────────────

pub struct AgentCtx {
    pub api_key: String,
    pub model_id: String,
    pub max_tokens: u64,
    /// Effective maximum tool-call rounds: min(request.max_turns, plan.max_turns).
    pub max_rounds: usize,

    pub thread_id: Option<String>,
    /// True when `thread_id` is Some and the loaded history was empty on this turn.
    pub thread_was_new: bool,
    pub tenant_id: String,
    pub tools: Vec<Value>,
    pub messages: Vec<AgentMessage>,
    pub effective_system: Option<String>,
    /// Parsed workspace node ULID, used to index chat content for search.
    pub workspace_node_id: Option<_Ulid>,
    /// Step 8.1 — original attachment object keys, forwarded to the projection job
    /// to record `linked_file_ids` in the workspace node metadata.
    pub attachment_ids: Vec<String>,
    pub max_invokes_per_turn: usize,
    /// Routing metadata emitted as the first SSE delta in streaming turns (PR 3.B).
    pub routing_meta: Value,
}

// ── build_ctx ─────────────────────────────────────────────────────────────────

pub async fn build_ctx(
    state: &Arc<AppState>,
    tenant: &ResolvedTenant,
    limits: PlanLimits,
    req: &crate::routes::chat::ChatRequest,
) -> Result<AgentCtx, HttpError> {
    let api_key = std::env::var("ANTHROPIC_API_KEY").unwrap_or_default();
    if api_key.trim().is_empty() {
        return Err(map_rig_error(
            "ANTHROPIC_API_KEY is not configured; set it before starting agent-gateway",
        ));
    }

    // ── Step 1: resolve ModelSpec from catalog ────────────────────────────────
    let spec = state
        .model_catalog
        .resolve_allowed(&tenant.0.plan, req.model.as_deref())
        .map_err(|e| match e {
            ModelError::NotFound(m) => {
                HttpError::validation("model", format!("unknown model: {m}"))
            }
            ModelError::PlanGated(m, p) => HttpError::validation(
                "model",
                format!("model '{m}' is not available on plan '{p}'"),
            ),
            ModelError::StreamingNotSupported(m) => {
                HttpError::validation("stream", format!("model '{m}' does not support streaming"))
            }
        })?;

    let model_id = spec.id.clone();

    // ── Step 2: streaming compatibility ──────────────────────────────────────
    if req.stream.unwrap_or(false) && !spec.supports_streaming {
        return Err(HttpError::validation(
            "stream",
            format!("model '{model_id}' does not support streaming"),
        ));
    }

    // ── Step 3: lightweight ToolRoutingDecision ───────────────────────────────
    let routing_decision = ToolRoutingDecision::from_request(
        req.forced_capability.as_deref(),
        !req.attachment_content.is_empty(),
    );

    // ── Step 4: gate on spec.supports_tools ──────────────────────────────────
    let skip_tools = if !spec.supports_tools {
        if routing_decision.tool_required {
            use agent_core::ToolRequirementReason;
            let msg = match routing_decision.reason {
                Some(ToolRequirementReason::ForcedCapability) => {
                    "selected model does not support tools".to_string()
                }
                _ => "task requires tools; selected model is text-only".to_string(),
            };
            return Err(HttpError::validation("model", msg));
        }
        true
    } else {
        false
    };

    // ── Step 5: reject vision attachments when model has no vision support ────
    if !spec.supports_vision && !req.attachment_content.is_empty() {
        let has_image = req.attachment_content.iter().any(|block| {
            block
                .get("type")
                .and_then(|t| t.as_str())
                .map(|t| t == "image")
                .unwrap_or(false)
        });
        if has_image {
            return Err(HttpError::validation(
                "attachment_content",
                format!("model '{model_id}' does not support vision/image attachments"),
            ));
        }
    }

    Span::current().record("gen_ai.request.model", model_id.as_str());

    let catalog = state.plan_catalog.by_tier(&tenant.0.plan);
    let catalog_max_tokens = catalog.map(|p| p.max_tokens).unwrap_or(limits.max_tokens);
    let catalog_max_turns = catalog
        .and_then(|p| p.max_turns_per_day)
        .unwrap_or(limits.max_turns as u64) as usize;

    let max_tokens = req
        .max_tokens
        .unwrap_or(catalog_max_tokens)
        .min(catalog_max_tokens);

    let tenant_id = tenant.0.tenant_id.to_string();
    let thread_store = Arc::clone(&state.thread_store);

    let max_rounds = req
        .max_turns
        .map(|s| (s as usize).min(catalog_max_turns))
        .unwrap_or(catalog_max_turns);

    // ── Step 2.9: resolve thread_id with cross-tenant ownership check ─────────
    //
    // 1. Explicit `thread_id` → verify it belongs to this tenant (→ 404 if not).
    // 2. workspace_node_id → resolve via ConversationService.
    // 3. Fallback → create floating thread.
    let thread_id: Option<String> = if let Some(tid) = req.thread_id.clone() {
        // Step 2.9: ownership check — reject cross-tenant thread access.
        match thread_store.get(&tenant_id, &tid).await {
            Ok(Some(_)) => {} // thread exists and belongs to this tenant
            Ok(None) => {
                return Err(HttpError::not_found("thread_id"));
            }
            Err(e) => {
                warn!(error = %e, tid, "thread ownership check failed");
                return Err(HttpError::not_found("thread_id"));
            }
        }
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

    // ── Step 2.9: workspace_node_id cross-tenant check ────────────────────────
    if let Some(ref node_id_str) = req.workspace_node_id
        && let Ok(node_id) = node_id_str.parse::<_Ulid>()
    {
        let user_id = tenant.0.user_id.as_deref().unwrap_or(tenant_id.as_str());
        match state
            .workspace_store
            .get_accessible_node(&tenant_id, user_id, node_id)
            .await
        {
            Ok(_) => {} // accessible — proceed
            Err(_) => {
                return Err(HttpError::forbidden(
                    "workspace node not accessible for this tenant",
                ));
            }
        }
    }

    // Load thread history.
    let mut history: Vec<AgentMessage> = if let Some(ref tid) = thread_id {
        match thread_store.messages(&tenant_id, tid).await {
            Ok(msgs) => msgs
                .iter()
                .map(|m| AgentMessage {
                    role: if m.role == "user" {
                        MessageRole::User
                    } else {
                        MessageRole::Assistant
                    },
                    content: MessageContent::Text(m.content.clone()),
                })
                .collect(),
            Err(e) => {
                warn!(error = %e, "failed to load thread history");
                vec![]
            }
        }
    } else {
        vec![]
    };

    let thread_was_new = thread_id.is_some() && history.is_empty();

    // ── Capability routing pipeline ───────────────────────────────────────────

    let user_query = req
        .messages
        .iter()
        .rfind(|m| m.role == "user")
        .map(|m| m.content.as_str())
        .unwrap_or("");
    let router_total_start = Instant::now();

    let (mut tools, max_score) = if skip_tools {
        (vec![], 0.0_f64)
    } else {
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
            Ok((defs, score)) => (defs, score),
            Err(e) => {
                warn!(error = %e, "semantic router failed — continuing with zero tools");
                (vec![], 0.0_f64)
            }
        }
    };

    let lexical_hits: Vec<String> = if skip_tools {
        vec![]
    } else {
        let stage_start = Instant::now();
        let hits = {
            let registry = state.registry.read();
            registry.lexical_hint_capabilities(user_query, &tenant_id)
        };
        let stage_ms = stage_start.elapsed().as_secs_f64() * 1000.0;
        if let Some(rm) = state.router_metrics.as_ref() {
            rm.observe_stage_ms("lexical", stage_ms);
        }
        hits
    };

    let semantic_cap_names: HashSet<String> = tools
        .iter()
        .filter_map(|v| {
            v.get("name")?
                .as_str()
                .and_then(|n| n.split_once("__").map(|(cap, _)| cap.to_string()))
        })
        .collect();
    let mut lex_only: Vec<Value> = Vec::new();
    for lex_cap in &lexical_hits {
        if !semantic_cap_names.contains(lex_cap.as_str()) {
            let cap_defs: Vec<Value> = {
                let registry = state.registry.read();
                registry
                    .tools_for_capability_exact_for_tenant(lex_cap, &tenant_id)
                    .unwrap_or_default()
            };
            lex_only.extend(cap_defs);
        }
    }
    if !lex_only.is_empty() {
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

    let max_tools = state.router_quota.max_tools_per_turn.max(1);
    let forced_cap_outcome: &'static str = if let Some(ref cap_name) = req.forced_capability {
        let stage_start = Instant::now();
        let pinned_tools: Option<Vec<Value>> = {
            let registry = state.registry.read();
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
                if tools.len() > max_tools {
                    tools.truncate(max_tools);
                }
                "dropped"
            }
            None => {
                // Step 2.9: unknown forced_capability → 400 (replaces silent warn).
                if !skip_tools {
                    return Err(HttpError::bad_request(format!(
                        "forced_capability '{}' is not available or not enabled for this tenant",
                        cap_name
                    )));
                }
                if tools.len() > max_tools {
                    tools.truncate(max_tools);
                }
                "dropped"
            }
        }
    } else {
        if tools.len() > max_tools {
            tools.truncate(max_tools);
        }
        "none"
    };

    let low_confidence = !skip_tools && max_score < state.router_quota.min_confidence;
    if low_confidence {
        if let Some(rm) = state.router_metrics.as_ref() {
            rm.record_low_confidence_turn();
        }
        warn!(
            max_score,
            threshold = state.router_quota.min_confidence,
            "router: low-confidence turn"
        );
    }

    if let Some(rm) = state.router_metrics.as_ref() {
        let total_ms = router_total_start.elapsed().as_secs_f64() * 1000.0;
        rm.observe_stage_ms("merge", total_ms);
        rm.observe_stage_ms("total", total_ms);
        rm.observe_tools_per_turn(tools.len());
        rm.record_forced_capability(forced_cap_outcome);
    }

    let selected_capabilities: Vec<String> = {
        let mut out: Vec<String> = Vec::new();
        for t in &tools {
            if let Some(name) = t.get("name").and_then(|v| v.as_str()) {
                let cap = name
                    .split_once("__")
                    .map(|(c, _)| c)
                    .unwrap_or(name)
                    .to_string();
                if !out.contains(&cap) {
                    out.push(cap);
                }
            }
        }
        out
    };
    let pinned_tools: Vec<String> = if forced_cap_outcome == "pinned_kept" {
        if let Some(ref cap_name) = req.forced_capability {
            let registry = state.registry.read();
            registry
                .tools_for_capability_exact_for_tenant(cap_name, &tenant_id)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|v| v.get("name")?.as_str().map(|s| s.to_string()))
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
    let last_user_pos = non_system.iter().rposition(|m| m.role == "user");

    let new_messages: Vec<AgentMessage> = non_system
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let role = if m.role == "user" {
                MessageRole::User
            } else {
                MessageRole::Assistant
            };
            if m.role == "user" && Some(i) == last_user_pos && !req.attachment_content.is_empty() {
                let mut blocks: Vec<ContentBlock> = req
                    .attachment_content
                    .iter()
                    .map(|v| ContentBlock::Raw(v.clone()))
                    .collect();
                if !m.content.is_empty() {
                    blocks.push(ContentBlock::Text {
                        text: m.content.clone(),
                    });
                }
                AgentMessage {
                    role,
                    content: MessageContent::Blocks(blocks),
                }
            } else {
                AgentMessage {
                    role,
                    content: MessageContent::Text(m.content.clone()),
                }
            }
        })
        .collect();

    const TOOL_GUARD: &str = "Only call tools when the user's request explicitly requires them. \
        For general conversation, questions, or anything that can be answered directly, \
        respond without invoking tools.";

    let system_content = req
        .messages
        .iter()
        .find(|m| m.role == "system")
        .map(|m| m.content.clone());

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

    let effective_system = if let Some(ref node_id_str) = req.workspace_node_id {
        if let Ok(node_id) = node_id_str.parse::<_Ulid>() {
            // Step 6.3 — sibling bias: default off; enable with CONUS_WORKSPACE_SIBLING_BIAS=1.
            let sibling_bias = std::env::var("CONUS_WORKSPACE_SIBLING_BIAS")
                .map(|v| matches!(v.as_str(), "1" | "true" | "yes"))
                .unwrap_or(false);
            let ctx_builder = ContextBuilder::new(
                Arc::clone(&state.workspace_store),
                Arc::clone(&state.workspace_content),
            )
            .with_sibling_bias(sibling_bias);
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

    // Persist incoming user messages to thread.
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

    // Conservative input token estimation — reject early.
    let msg_json_len: usize = history.iter().map(|m| m.estimated_bytes()).sum();
    let sys_len = effective_system.as_deref().map(|s| s.len()).unwrap_or(0);
    let tools_json_len = serde_json::to_string(&tools).map(|s| s.len()).unwrap_or(0);
    let token_estimate = estimate_input_tokens(msg_json_len, sys_len, tools_json_len);
    if token_estimate_exceeds_limit(token_estimate, spec) {
        common::metrics::record_llm_input_token_estimate_exceeded(
            &spec.provider.to_string(),
            &spec.id,
        );
        return Err(HttpError::validation(
            "messages",
            format!(
                "estimated input ({token_estimate} tokens) exceeds model '{}' limit ({} tokens)",
                spec.id, spec.max_input_tokens,
            ),
        ));
    }

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
        // Step 8.1 — forward original attachment object keys for metadata recording.
        attachment_ids: req.attachment_ids.clone(),
        max_invokes_per_turn: state.router_quota.max_invokes_per_turn.max(1),
        routing_meta,
    })
}

// ── merge_pinned ──────────────────────────────────────────────────────────────

/// Merge `pinned` tool definitions before `semantic` ones (PR 2.A.3).
///
/// 1. Start with `pinned` (always at the front).
/// 2. Append semantic tools not already in `pinned` (dedup by name).
/// 3. Truncate to `max_tools` — pinned tools survive.
pub fn merge_pinned(pinned: Vec<Value>, semantic: Vec<Value>, max_tools: usize) -> Vec<Value> {
    let seen: HashSet<String> = pinned
        .iter()
        .filter_map(|v| v.get("name")?.as_str().map(|s| s.to_string()))
        .collect();

    let mut result = pinned;
    for tool in semantic {
        let name = tool
            .get("name")
            .and_then(|n| n.as_str())
            .map(|s| s.to_string());
        if name.as_deref().map(|n| !seen.contains(n)).unwrap_or(true) {
            result.push(tool);
        }
    }
    result.truncate(max_tools);
    result
}
