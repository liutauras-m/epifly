//! `AgentTurnRunner` — unified agent loop for blocking and streaming paths.
//!
//! Steps 2.5, 2.6, 2.7: typed event system, provider abstraction, unified loop.
//!
//! # Architecture
//!
//! ```text
//!   routes/agent.rs           agent/runner.rs              agent/provider/*
//!   ─────────────────         ──────────────────            ────────────────
//!   blocking_agent()  ──┐
//!                       ├──► AgentTurnRunner::run()  ──►  AgentProvider::stream_events()
//!   stream_agent()  ────┘         │                                  │
//!                                 │ (via internal channel)           │
//!                                 ◄──────────────────────────────────┘
//!                                 │ emits AgentEvent to:
//!                             BlockingSink  (accumulates text)
//!                             SseSink       (sends SSE to client)
//! ```
//!
//! The runner spawns the provider stream into a `tokio::mpsc` channel and
//! processes `ProviderEvent`s concurrently so text deltas reach the sink
//! word-by-word (not buffered per round).

use crate::agent::{
    metering::record_agent_usage,
    persistence::{enqueue_projection_job, maybe_set_title, persist_assistant_message},
    prompt_hooks::{PromptHook, Usage},
    provider::{AgentProvider, ProviderEvent, ProviderEventSink, ProviderRequest},
    tool_execution::{resolve_and_invoke, truncate_tool_result},
};
use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use agent_core::{
    AgentMessage, ContentBlock, MessageContent, MessageRole, WorkspaceChangeEvent,
    realtime::invalidation::InvalidationEvent,
};
use async_trait::async_trait;
use axum::response::sse::Event;
use serde_json::{Value, json};
use std::collections::HashMap;

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{Span, info, warn};
use uuid::Uuid;

#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

use super::context::AgentCtx;
use super::provider::ProviderError;

// ── AgentEvent ────────────────────────────────────────────────────────────────

/// Typed events emitted by `AgentTurnRunner::run()` through `AgentEventSink`.
#[derive(Debug)]
pub enum AgentEvent {
    /// Routing metadata (always the first event emitted).
    RoutingMeta(Value),
    /// Incremental text chunk from the model.
    TextDelta(String),
    /// A tool call has started (name shown to the user before execution).
    ToolStart { id: String, name: String },
    /// A tool call has completed and the result is available.
    ToolResult {
        tool_use_id: String,
        name: String,
        result: String,
    },
    /// Workspace resources were mutated; clients should re-fetch.
    ResourceInvalidated {
        resource: String,
        scope: String,
        changed_keys: Vec<String>,
    },
    /// Turn complete. Contains accumulated usage.
    Done {
        completion_id: String,
        model: String,
        thread_id: Option<String>,
        input_tokens: u64,
        output_tokens: u64,
        tool_calls_made: usize,
    },
}

// ── AgentEmitError ────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum AgentEmitError {
    /// The downstream SSE channel was closed — client disconnected.
    ClientGone,
    Other(anyhow::Error),
}

impl std::fmt::Display for AgentEmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentEmitError::ClientGone => write!(f, "client disconnected"),
            AgentEmitError::Other(e) => write!(f, "{e}"),
        }
    }
}

// ── AgentEventSink ────────────────────────────────────────────────────────────

#[async_trait]
pub trait AgentEventSink: Send {
    async fn emit(&mut self, ev: AgentEvent) -> Result<(), AgentEmitError>;
}

// ── AgentError ────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("provider error: {0}")]
    Provider(#[from] ProviderError),
    #[error("max rounds exceeded")]
    MaxRoundsExceeded,
    #[error("client disconnected")]
    ClientGone,
    #[error("{0}")]
    Config(String),
}

// ── BlockingSink ──────────────────────────────────────────────────────────────

/// Accumulates the complete agent response for the blocking JSON path.
pub struct BlockingSink {
    pub text: String,
    pub tool_calls_made: usize,
    pub total_input: u64,
    pub total_output: u64,
    pub thread_id_after: Option<String>,
    pub completion_id: String,
    pub model: String,
}

impl BlockingSink {
    pub fn new(completion_id: String, model: String, thread_id: Option<String>) -> Self {
        Self {
            text: String::new(),
            tool_calls_made: 0,
            total_input: 0,
            total_output: 0,
            thread_id_after: thread_id,
            completion_id,
            model,
        }
    }
}

#[async_trait]
impl AgentEventSink for BlockingSink {
    async fn emit(&mut self, ev: AgentEvent) -> Result<(), AgentEmitError> {
        match ev {
            AgentEvent::TextDelta(text) => self.text.push_str(&text),
            AgentEvent::ToolResult { .. } => {
                self.tool_calls_made += 1;
            }
            AgentEvent::Done {
                thread_id,
                input_tokens,
                output_tokens,
                tool_calls_made,
                ..
            } => {
                self.thread_id_after = thread_id;
                self.total_input = input_tokens;
                self.total_output = output_tokens;
                self.tool_calls_made = tool_calls_made;
            }
            // RoutingMeta, ToolStart, ResourceInvalidated ignored for blocking JSON response.
            _ => {}
        }
        Ok(())
    }
}

// ── SseSink ───────────────────────────────────────────────────────────────────

/// Forwards `AgentEvent`s to an SSE channel as OpenAI-compatible JSON chunks.
pub struct SseSink {
    tx: mpsc::Sender<Result<Event, Infallible>>,
    completion_id: String,
    model: String,
}

impl SseSink {
    pub fn new(
        tx: mpsc::Sender<Result<Event, Infallible>>,
        completion_id: String,
        model: String,
    ) -> Self {
        Self {
            tx,
            completion_id,
            model,
        }
    }

    /// Send a plain-text error delta followed by `finish_reason=stop` and `[DONE]`.
    pub async fn send_error(&self, message: &str, thread_id: Option<&str>) {
        let text = format!("Error: {message}");
        let _ = self
            .tx
            .send(Ok(Event::default().data(
                json!({
                    "id": self.completion_id,
                    "object": "chat.completion.chunk",
                    "model": self.model,
                    "choices": [{"index": 0, "delta": {"content": text}, "finish_reason": null}],
                    "thread_id": thread_id,
                })
                .to_string(),
            )))
            .await;
        let _ = self
            .tx
            .send(Ok(Event::default().data(
                json!({
                    "id": self.completion_id,
                    "object": "chat.completion.chunk",
                    "model": self.model,
                    "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
                    "thread_id": thread_id,
                })
                .to_string(),
            )))
            .await;
        let _ = self.tx.send(Ok(Event::default().data("[DONE]"))).await;
    }
}

#[async_trait]
impl AgentEventSink for SseSink {
    async fn emit(&mut self, ev: AgentEvent) -> Result<(), AgentEmitError> {
        let chunk = match ev {
            AgentEvent::RoutingMeta(meta) => json!({
                "id": self.completion_id,
                "object": "chat.completion.chunk",
                "model": self.model,
                "choices": [{"index": 0, "delta": {"routing_meta": meta}, "finish_reason": null}],
            }),
            AgentEvent::TextDelta(text) => json!({
                "id": self.completion_id,
                "object": "chat.completion.chunk",
                "model": self.model,
                "choices": [{"index": 0, "delta": {"content": text}, "finish_reason": null}],
            }),
            AgentEvent::ToolStart { id, name } => json!({
                "id": self.completion_id,
                "object": "chat.completion.chunk",
                "model": self.model,
                "choices": [{"index": 0, "delta": {"tool_call_start": {"id": id, "name": name}}, "finish_reason": null}],
            }),
            AgentEvent::ToolResult {
                tool_use_id,
                name,
                result,
            } => json!({
                "id": self.completion_id,
                "object": "chat.completion.chunk",
                "model": self.model,
                "choices": [{"index": 0, "delta": {"tool_call_result": {"tool_use_id": tool_use_id, "name": name, "result": result}}, "finish_reason": null}],
            }),
            AgentEvent::ResourceInvalidated {
                resource,
                scope,
                changed_keys,
            } => json!({
                "id": self.completion_id,
                "object": "chat.completion.chunk",
                "model": self.model,
                "choices": [{"index": 0, "delta": {"resource_invalidated": {"resource": resource, "scope": scope, "changed_keys": changed_keys}}, "finish_reason": null}],
            }),
            AgentEvent::Done {
                thread_id,
                input_tokens,
                output_tokens,
                tool_calls_made,
                ..
            } => {
                let usage_chunk = json!({
                    "id": self.completion_id,
                    "object": "chat.completion.chunk",
                    "model": self.model,
                    "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
                    "usage": {
                        "prompt_tokens": input_tokens,
                        "completion_tokens": output_tokens,
                        "total_tokens": input_tokens + output_tokens,
                    },
                    "tool_calls_made": tool_calls_made,
                    "thread_id": thread_id,
                });
                // Emit usage chunk then [DONE].
                if self
                    .tx
                    .send(Ok(Event::default().data(usage_chunk.to_string())))
                    .await
                    .is_err()
                {
                    return Err(AgentEmitError::ClientGone);
                }
                return self
                    .tx
                    .send(Ok(Event::default().data("[DONE]")))
                    .await
                    .map_err(|_| AgentEmitError::ClientGone);
            }
        };

        self.tx
            .send(Ok(Event::default().data(chunk.to_string())))
            .await
            .map_err(|_| AgentEmitError::ClientGone)
    }
}

// ── ChannelSink (private) ─────────────────────────────────────────────────────

/// Internal sink used by the runner to pipe provider events through an mpsc channel
/// so they can be consumed concurrently from the processing loop.
struct ChannelSink {
    tx: mpsc::Sender<ProviderEvent>,
}

#[async_trait]
impl ProviderEventSink for ChannelSink {
    async fn on_event(&mut self, ev: ProviderEvent) -> Result<(), ProviderError> {
        self.tx
            .send(ev)
            .await
            .map_err(|_| ProviderError::Transport("rx closed".into()))
    }
}

// ── AgentTurnRunner ───────────────────────────────────────────────────────────

pub struct AgentTurnRunner {
    state: Arc<AppState>,
    tenant: ResolvedTenant,
    pub ctx: AgentCtx,
    provider: Arc<dyn AgentProvider>,
    /// Prompt hooks run before and after each turn (in registration order).
    hooks: Vec<Arc<dyn PromptHook>>,
}

impl AgentTurnRunner {
    pub fn new(
        state: Arc<AppState>,
        tenant: ResolvedTenant,
        ctx: AgentCtx,
        provider: Arc<dyn AgentProvider>,
    ) -> Self {
        Self {
            state,
            tenant,
            ctx,
            provider,
            hooks: vec![],
        }
    }

    /// Attach a prompt hook. Hooks run in registration order.
    pub fn add_hook(mut self, hook: Arc<dyn PromptHook>) -> Self {
        self.hooks.push(hook);
        self
    }

    /// Execute the agent loop, emitting typed events through `sink`.
    ///
    /// Returns `Ok(())` on success or graceful cancellation.
    /// Returns `Err(AgentError)` on provider failures or exceeded round limit.
    pub async fn run(
        &mut self,
        sink: &mut dyn AgentEventSink,
        cancel: CancellationToken,
    ) -> Result<(), AgentError> {
        let start = Instant::now();
        let mut total_input = 0u64;
        let mut total_output = 0u64;
        let mut tool_calls_made = 0usize;
        let mut all_changed_paths: Vec<String> = vec![];
        let mut title_was_set = false;
        let mut full_assistant_text = String::new();

        let completion_id = format!("chatcmpl-{}", Uuid::new_v4());

        Span::current().record("gen_ai.request.model", self.ctx.model_id.as_str());

        // Run before_turn hooks (e.g. EnforceMaxInputHook, RedactPiiHook).
        for hook in &self.hooks {
            if let Err(e) = hook.before_turn(&mut self.ctx).await {
                return Err(AgentError::Config(e.to_string()));
            }
        }

        // Routing metadata is always the first event (PR 3.B).
        if let Err(AgentEmitError::ClientGone) = sink
            .emit(AgentEvent::RoutingMeta(self.ctx.routing_meta.clone()))
            .await
        {
            cancel.cancel();
            return Ok(());
        }

        'rounds: for round in 0..self.ctx.max_rounds {
            if cancel.is_cancelled() {
                return Ok(());
            }

            let req = ProviderRequest {
                model: self.ctx.model_id.clone(),
                max_tokens: self.ctx.max_tokens,
                messages: self.ctx.messages.clone(),
                tools: self.ctx.tools.clone(),
                system: self.ctx.effective_system.clone(),
            };

            info!(round, model = self.ctx.model_id, "agent loop iteration");

            // Pipe provider stream through an internal channel so text deltas
            // reach the sink in real-time (not buffered per round).
            let (ev_tx, mut ev_rx) = mpsc::channel::<ProviderEvent>(64);
            let provider = Arc::clone(&self.provider);
            let cancel_clone = cancel.clone();
            let req_id = Uuid::new_v4();

            let stream_handle = tokio::spawn(async move {
                let mut chan_sink = ChannelSink { tx: ev_tx };
                provider
                    .stream_events(req, &mut chan_sink, cancel_clone, req_id)
                    .await
            });

            // Accumulated per-round state.
            let mut stop_reason = String::new();
            // index → (id, name, accumulated_json)
            let mut tool_blocks: HashMap<usize, (String, String, String)> = HashMap::new();
            let mut round_input = 0u64;
            let mut round_output = 0u64;
            let mut current_text = String::new();

            'events: while let Some(ev) = ev_rx.recv().await {
                if cancel.is_cancelled() {
                    break 'events;
                }
                match ev {
                    ProviderEvent::InputUsage { input_tokens } => {
                        round_input += input_tokens;
                    }
                    ProviderEvent::TextDelta(text) => {
                        current_text.push_str(&text);
                        full_assistant_text.push_str(&text);
                        match sink.emit(AgentEvent::TextDelta(text)).await {
                            Ok(()) => {}
                            Err(AgentEmitError::ClientGone) => {
                                cancel.cancel();
                                break 'events;
                            }
                            Err(_) => {}
                        }
                    }
                    ProviderEvent::ToolStart { index, id, name } => {
                        tool_blocks.insert(index, (id.clone(), name.clone(), String::new()));
                        let _ = sink.emit(AgentEvent::ToolStart { id, name }).await;
                    }
                    ProviderEvent::ToolInputDelta {
                        index,
                        partial_json,
                    } => {
                        if let Some(entry) = tool_blocks.get_mut(&index) {
                            entry.2.push_str(&partial_json);
                        }
                    }
                    ProviderEvent::ContentBlockStop(idx) => {
                        // If this was a text block, push the accumulated text.
                        if !tool_blocks.contains_key(&idx) && !current_text.is_empty() {
                            current_text = String::new();
                        }
                    }
                    ProviderEvent::MessageDelta {
                        output_tokens,
                        stop_reason: sr,
                    } => {
                        round_output += output_tokens;
                        stop_reason = sr;
                    }
                    ProviderEvent::Done => break 'events,
                }
            }

            // Wait for the provider task (cleanup, surface errors).
            match stream_handle.await {
                Ok(Err(e)) if !cancel.is_cancelled() => {
                    return Err(AgentError::Provider(e));
                }
                _ => {}
            }

            if cancel.is_cancelled() {
                return Ok(());
            }

            total_input += round_input;
            total_output += round_output;

            Span::current().record("gen_ai.usage.input_tokens", total_input);
            Span::current().record("gen_ai.usage.output_tokens", total_output);

            if stop_reason != "tool_use" {
                // Final turn — persist, index, meter, broadcast, emit events.
                if full_assistant_text.is_empty() && tool_calls_made == 0 {
                    return Err(AgentError::Config(
                        "upstream stream ended without any assistant content".into(),
                    ));
                }

                if let Some(ref tid) = self.ctx.thread_id {
                    persist_assistant_message(
                        &self.state,
                        &self.ctx.tenant_id,
                        tid,
                        &full_assistant_text,
                    )
                    .await;

                    title_was_set = maybe_set_title(
                        &self.state.thread_store,
                        &self.ctx.tenant_id,
                        tid,
                        &full_assistant_text,
                    )
                    .await;

                    if let Some(node_id) = self.ctx.workspace_node_id {
                        // Step 8.1 — forward attachment object keys for linked_file_ids metadata.
                        enqueue_projection_job(
                            &self.state,
                            self.ctx.tenant_id.clone(),
                            tid.clone(),
                            node_id,
                            self.ctx.attachment_ids.clone(),
                        );
                    }
                }

                info!(
                    input_tokens = total_input,
                    output_tokens = total_output,
                    tool_calls = tool_calls_made,
                    "agent loop complete"
                );

                let duration_ms = start.elapsed().as_millis() as u64;

                // Run after_turn hooks (e.g. LogTokensHook).
                let usage = Usage {
                    input_tokens: total_input,
                    output_tokens: total_output,
                    tool_calls_made,
                    duration_ms,
                };
                for hook in &self.hooks {
                    let _ = hook.after_turn(&self.ctx, &usage).await;
                }

                record_agent_usage(
                    &self.state,
                    &self.ctx.tenant_id,
                    &self.ctx.model_id,
                    total_input,
                    total_output,
                    tool_calls_made,
                    duration_ms,
                )
                .await;

                // Emit workspace invalidation events.
                if !all_changed_paths.is_empty() {
                    let mut seen = std::collections::HashSet::new();
                    let deduped: Vec<String> = all_changed_paths
                        .iter()
                        .filter(|p| seen.insert(p.as_str()))
                        .cloned()
                        .collect();

                    let _ = sink
                        .emit(AgentEvent::ResourceInvalidated {
                            resource: "workspace".into(),
                            scope: self.ctx.tenant_id.clone(),
                            changed_keys: deduped.clone(),
                        })
                        .await;

                    let _ = self.state.invalidation_bus.send(
                        InvalidationEvent::new("workspace", &self.ctx.tenant_id)
                            .with_keys(deduped.clone()),
                    );
                    self.state
                        .realtime_service
                        .publish_workspace_change(WorkspaceChangeEvent {
                            op: "workspace.invalidated".into(),
                            tenant_id: self.ctx.tenant_id.clone(),
                            node_id: deduped.first().cloned().unwrap_or_else(|| "*".into()),
                            kind: "workspace".into(),
                        })
                        .await;
                }

                // Emit threads invalidation when the list is likely to have changed.
                if (self.ctx.thread_was_new || title_was_set)
                    && let Some(ref tid) = self.ctx.thread_id
                {
                    let _ = sink
                        .emit(AgentEvent::ResourceInvalidated {
                            resource: "threads".into(),
                            scope: self.ctx.tenant_id.clone(),
                            changed_keys: vec![tid.clone()],
                        })
                        .await;

                    let _ = self.state.invalidation_bus.send(
                        InvalidationEvent::new("threads", &self.ctx.tenant_id)
                            .with_keys(vec![tid.clone()]),
                    );
                    self.state
                        .realtime_service
                        .publish_workspace_change(WorkspaceChangeEvent {
                            op: "threads.invalidated".into(),
                            tenant_id: self.ctx.tenant_id.clone(),
                            node_id: tid.clone(),
                            kind: "thread".into(),
                        })
                        .await;
                }

                let _ = sink
                    .emit(AgentEvent::Done {
                        completion_id: completion_id.clone(),
                        model: self.ctx.model_id.clone(),
                        thread_id: self.ctx.thread_id.clone(),
                        input_tokens: total_input,
                        output_tokens: total_output,
                        tool_calls_made,
                    })
                    .await;

                return Ok(());
            }

            // ── Tool execution round ──────────────────────────────────────────

            // Build typed assistant message: text + tool_use blocks.
            let mut assistant_blocks: Vec<ContentBlock> = Vec::new();
            if !current_text.is_empty() {
                assistant_blocks.push(ContentBlock::Text {
                    text: current_text.clone(),
                });
            }

            let mut sorted_blocks: Vec<_> = tool_blocks.into_iter().collect();
            sorted_blocks.sort_by_key(|(idx, _)| *idx);

            let mut invalid_tool_inputs: HashMap<String, String> = HashMap::new();

            for (_, (id, name, json_str)) in &sorted_blocks {
                let parsed: Value = match serde_json::from_str(json_str) {
                    Ok(v) => v,
                    Err(e) => {
                        invalid_tool_inputs.insert(id.clone(), e.to_string());
                        json!({ "_invalid_json": json_str })
                    }
                };
                assistant_blocks.push(ContentBlock::ToolUse {
                    id: id.clone(),
                    name: name.clone(),
                    input: parsed,
                });
            }
            self.ctx.messages.push(AgentMessage {
                role: MessageRole::Assistant,
                content: MessageContent::Blocks(assistant_blocks),
            });

            let mut tool_result_blocks: Vec<ContentBlock> = Vec::new();
            let mut invoke_limit_hit = false;

            for (_, (id, name, json_str)) in sorted_blocks {
                // Invalid JSON — inject error result immediately.
                if let Some(msg) = invalid_tool_inputs.remove(&id) {
                    tool_result_blocks.push(ContentBlock::ToolResult {
                        tool_use_id: id,
                        content: format!("Invalid tool input JSON: {msg}"),
                        is_error: true,
                    });
                    continue;
                }

                if invoke_limit_hit || tool_calls_made >= self.ctx.max_invokes_per_turn {
                    invoke_limit_hit = true;
                    tool_result_blocks.push(ContentBlock::ToolResult {
                        tool_use_id: id,
                        content: format!(
                            "Skipped: per-turn tool limit ({}) reached. \
                             Summarise what was completed so far and ask the user to reply \
                             'continue' to process the remaining items.",
                            self.ctx.max_invokes_per_turn
                        ),
                        is_error: true,
                    });
                    continue;
                }

                if cancel.is_cancelled() {
                    tool_result_blocks.push(ContentBlock::ToolResult {
                        tool_use_id: id,
                        content: "Skipped: request was cancelled.".into(),
                        is_error: true,
                    });
                    continue;
                }

                match serde_json::from_str(&json_str) {
                    Ok(parsed_input) => {
                        info!(round, tool = name, "executing tool");
                        tool_calls_made += 1;

                        let (result_str, paths) = match resolve_and_invoke(
                            &self.state,
                            &name,
                            &parsed_input,
                            &self.tenant,
                        )
                        .await
                        {
                            Ok((v, p)) => (v.to_string(), p),
                            Err(e) => {
                                warn!(tool = name, error = %e, "tool invocation failed");
                                (format!("Error: {e}"), vec![])
                            }
                        };

                        all_changed_paths.extend(paths);
                        let result_str = truncate_tool_result(
                            result_str,
                            &name,
                            self.state.router_quota.max_tool_result_bytes,
                        );

                        let _ = sink
                            .emit(AgentEvent::ToolResult {
                                tool_use_id: id.clone(),
                                name: name.clone(),
                                result: result_str.clone(),
                            })
                            .await;

                        tool_result_blocks.push(ContentBlock::ToolResult {
                            tool_use_id: id,
                            content: result_str,
                            is_error: false,
                        });
                    }
                    Err(e) => {
                        tool_result_blocks.push(ContentBlock::ToolResult {
                            tool_use_id: id,
                            content: format!("Invalid tool input JSON: {e}"),
                            is_error: true,
                        });
                        continue;
                    }
                }
            }

            self.ctx.messages.push(AgentMessage {
                role: MessageRole::User,
                content: MessageContent::Blocks(tool_result_blocks),
            });
            continue 'rounds;
        }

        Err(AgentError::MaxRoundsExceeded)
    }
}

// ── Step 4.4 — AgentTurnRunner property tests ─────────────────────────────────
//
// Property: for any tool_rounds in 0..=max_rounds-1, the runner terminates
// cleanly and emits exactly one `Done` event.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::context::AgentCtx;
    use crate::agent::provider::{
        AgentProvider, ProviderError, ProviderEvent, ProviderEventSink, ProviderRequest,
        ProviderResponse,
    };
    use agent_core::{AgentMessage, MessageContent, MessageRole, PlanTier, TenantContext};
    use async_trait::async_trait;
    use proptest::prelude::*;
    use std::path::PathBuf;

    // ── MockProvider ──────────────────────────────────────────────────────────

    /// Emits `tool_rounds` rounds of `stop_reason="tool_use"` then one text round.
    struct MockProvider {
        tool_rounds: usize,
        calls: AtomicUsize,
    }

    impl MockProvider {
        fn new(tool_rounds: usize) -> Arc<Self> {
            Arc::new(Self {
                tool_rounds,
                calls: AtomicUsize::new(0),
            })
        }
    }

    #[async_trait]
    impl AgentProvider for MockProvider {
        async fn complete(
            &self,
            _req: ProviderRequest,
            _id: Uuid,
        ) -> Result<ProviderResponse, ProviderError> {
            unimplemented!("blocking path not exercised in runner tests")
        }

        async fn stream_events(
            &self,
            _req: ProviderRequest,
            sink: &mut dyn ProviderEventSink,
            _cancel: CancellationToken,
            _req_id: Uuid,
        ) -> Result<(), ProviderError> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            sink.on_event(ProviderEvent::InputUsage { input_tokens: 1 })
                .await?;
            if call < self.tool_rounds {
                sink.on_event(ProviderEvent::ToolStart {
                    index: 0,
                    id: format!("tu_{call}"),
                    name: "mock__noop".into(),
                })
                .await?;
                sink.on_event(ProviderEvent::ToolInputDelta {
                    index: 0,
                    partial_json: "{}".into(),
                })
                .await?;
                sink.on_event(ProviderEvent::ContentBlockStop(0)).await?;
                sink.on_event(ProviderEvent::MessageDelta {
                    output_tokens: 1,
                    stop_reason: "tool_use".into(),
                })
                .await?;
            } else {
                sink.on_event(ProviderEvent::TextDelta("done".into()))
                    .await?;
                sink.on_event(ProviderEvent::ContentBlockStop(0)).await?;
                sink.on_event(ProviderEvent::MessageDelta {
                    output_tokens: 1,
                    stop_reason: "end_turn".into(),
                })
                .await?;
            }
            sink.on_event(ProviderEvent::Done).await?;
            Ok(())
        }
    }

    // ── CountSink ─────────────────────────────────────────────────────────────

    struct CountSink {
        done_count: usize,
    }

    impl CountSink {
        fn new() -> Self {
            Self { done_count: 0 }
        }
    }

    #[async_trait]
    impl AgentEventSink for CountSink {
        async fn emit(&mut self, ev: AgentEvent) -> Result<(), AgentEmitError> {
            if matches!(ev, AgentEvent::Done { .. }) {
                self.done_count += 1;
            }
            Ok(())
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn test_tenant() -> crate::mw::tenant::ResolvedTenant {
        crate::mw::tenant::ResolvedTenant(TenantContext::new(
            "test-tenant",
            None::<&str>,
            PlanTier::Enterprise,
            PathBuf::from("/tmp"),
        ))
    }

    fn minimal_ctx(max_rounds: usize) -> AgentCtx {
        AgentCtx {
            api_key: "test".into(),
            model_id: "claude-opus-4-7".into(),
            max_tokens: 1024,
            max_rounds,
            thread_id: None,
            thread_was_new: false,
            tenant_id: "test-tenant".into(),
            tools: vec![],
            messages: vec![AgentMessage {
                role: MessageRole::User,
                content: MessageContent::Text("hi".into()),
            }],
            effective_system: None,
            workspace_node_id: None,
            attachment_ids: vec![],
            max_invokes_per_turn: 10,
            routing_meta: serde_json::json!({}),
        }
    }

    async fn run_mock(tool_rounds: usize) -> (Result<(), AgentError>, usize) {
        let state =
            Arc::new(crate::state::AppState::with_in_memory_stores().expect("in-memory state"));
        let tenant = test_tenant();
        let ctx = minimal_ctx(tool_rounds + 2); // enough headroom to complete
        let provider = MockProvider::new(tool_rounds);
        let mut runner = AgentTurnRunner::new(state, tenant, ctx, provider);
        let mut sink = CountSink::new();
        let cancel = CancellationToken::new();
        let result = runner.run(&mut sink, cancel).await;
        (result, sink.done_count)
    }

    // ── Concrete tests ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn runner_zero_tool_rounds_emits_one_done() {
        let (result, done) = run_mock(0).await;
        assert!(result.is_ok(), "expected Ok; got {result:?}");
        assert_eq!(done, 1);
    }

    #[tokio::test]
    async fn runner_three_tool_rounds_emits_one_done() {
        let (result, done) = run_mock(3).await;
        assert!(result.is_ok(), "expected Ok; got {result:?}");
        assert_eq!(done, 1);
    }

    #[tokio::test]
    async fn runner_exceeds_max_rounds_returns_error_and_no_done() {
        let state = Arc::new(crate::state::AppState::with_in_memory_stores().unwrap());
        let tenant = test_tenant();
        // max_rounds=2 but the provider will always respond with tool_use.
        let ctx = minimal_ctx(2);
        let provider = MockProvider::new(99);
        let mut runner = AgentTurnRunner::new(state, tenant, ctx, provider);
        let mut sink = CountSink::new();
        let cancel = CancellationToken::new();
        let result = runner.run(&mut sink, cancel).await;
        assert!(
            matches!(result, Err(AgentError::MaxRoundsExceeded)),
            "expected MaxRoundsExceeded; got {result:?}"
        );
        assert_eq!(sink.done_count, 0, "error path must not emit Done");
    }

    #[tokio::test]
    async fn runner_respects_cancellation_before_first_round() {
        let state = Arc::new(crate::state::AppState::with_in_memory_stores().unwrap());
        let tenant = test_tenant();
        let ctx = minimal_ctx(5);
        let provider = MockProvider::new(99);
        let mut runner = AgentTurnRunner::new(state, tenant, ctx, provider);
        let mut sink = CountSink::new();
        let cancel = CancellationToken::new();
        cancel.cancel();
        let result = runner.run(&mut sink, cancel).await;
        assert!(
            result.is_ok(),
            "cancellation must return Ok; got {result:?}"
        );
        assert_eq!(sink.done_count, 0, "cancelled turn must not emit Done");
    }

    // ── Property test (Step 4.4) ───────────────────────────────────────────────

    proptest! {
        #[test]
        fn runner_terminates_with_exactly_one_done_for_any_valid_depth(
            tool_rounds in 0usize..=5
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let (result, done_count) = rt.block_on(run_mock(tool_rounds));
            prop_assert!(result.is_ok(), "runner returned error: {:?}", result);
            prop_assert_eq!(done_count, 1, "expected exactly 1 Done, got {}", done_count);
        }
    }

    // ── Step 5.8 — Persistence ordering property test ─────────────────────────
    //
    // Invariant (CLAUDE.md #15): Every `AgentEvent::Done` must be preceded by a
    // successful synchronous `append_message` (i.e. `persist_assistant_message`
    // must have returned before `Done` is emitted to the sink).
    //
    // We verify this by querying the in-memory thread store from *inside* the
    // sink's `emit` callback for `Done`: if the assistant message is not yet
    // present in the store, the ordering invariant was violated.

    /// Sink that checks thread-store persistence ordering when `Done` arrives.
    struct PersistenceOrderSink {
        thread_store: Arc<dyn common::memory::ThreadStore>,
        tenant_id: String,
        thread_id: String,
        pub violations: usize,
    }

    #[async_trait]
    impl AgentEventSink for PersistenceOrderSink {
        async fn emit(&mut self, ev: AgentEvent) -> Result<(), AgentEmitError> {
            if matches!(ev, AgentEvent::Done { .. }) {
                let messages = self
                    .thread_store
                    .messages(&self.tenant_id, &self.thread_id)
                    .await
                    .unwrap_or_default();
                let has_assistant = messages.iter().any(|m| m.role == "assistant");
                if !has_assistant {
                    self.violations += 1;
                }
            }
            Ok(())
        }
    }

    /// Run the mock with a real thread_id so `persist_assistant_message` fires,
    /// then return the number of ordering violations detected.
    async fn run_with_persistence(tool_rounds: usize) -> usize {
        let state = Arc::new(crate::state::AppState::with_in_memory_stores().unwrap());
        let tenant = test_tenant();
        let mut ctx = minimal_ctx(tool_rounds + 2);
        // Use a fixed thread_id so the sink can query the same key.
        ctx.thread_id = Some("step-5-8-prop-thread".into());
        let provider = MockProvider::new(tool_rounds);
        let thread_store = Arc::clone(&state.thread_store);
        let mut runner = AgentTurnRunner::new(Arc::clone(&state), tenant, ctx, provider);
        let mut sink = PersistenceOrderSink {
            thread_store,
            tenant_id: "test-tenant".into(),
            thread_id: "step-5-8-prop-thread".into(),
            violations: 0,
        };
        let cancel = CancellationToken::new();
        let _ = runner.run(&mut sink, cancel).await;
        sink.violations
    }

    proptest! {
        /// Step 5.8: for any tool_rounds in 0..=3, `AgentEvent::Done` is only
        /// emitted after `persist_assistant_message` has returned (i.e. the
        /// assistant message is already in the thread store when `Done` fires).
        #[test]
        fn runner_done_only_after_persist_for_any_depth(tool_rounds in 0usize..=3) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let violations = rt.block_on(run_with_persistence(tool_rounds));
            prop_assert_eq!(
                violations, 0,
                "Done emitted before persist_assistant_message returned (tool_rounds={})",
                tool_rounds
            );
        }
    }
}
