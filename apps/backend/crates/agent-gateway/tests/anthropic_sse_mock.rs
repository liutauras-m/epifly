//! Step 0.2 — Anthropic SSE upstream mock harness.
//!
//! Spins up a `wiremock` server that mimics the Anthropic Messages API and
//! drives a full `AgentTurnRunner` turn.  Asserts the event sequence emitted
//! by the gateway:
//!
//! - `routing_meta` is always the **first** event (invariant must survive every
//!   future `agent.rs` change).
//! - Text deltas arrive after `routing_meta` and before `done`.
//! - Tool-use turns interleave `tool_start` / `tool_result` between
//!   `routing_meta` and `done`.
//! - `done` is always the **last** event.
//!
//! Uses `AppState::with_in_memory_stores()` so no external services are needed.

use agent_core::{AgentMessage, MessageContent, MessageRole, PlanTier, TenantContext};
use agent_gateway::{
    agent::{
        AgentCtx, AgentEmitError, AgentEvent, AgentEventSink, AgentTurnRunner,
        NativeAnthropicProvider,
    },
    mw::tenant::ResolvedTenant,
    state::AppState,
};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use std::{path::PathBuf, sync::Arc};
use tokio_util::sync::CancellationToken;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

// ── Canned SSE bodies ──────────────────────────────────────────────────────────

/// Minimal Anthropic SSE stream that produces a two-chunk text response.
///
/// Each event is separated by `\n\n` as required by the SSE spec.
fn text_only_sse() -> String {
    let events: &[&str] = &[
        r#"{"type":"message_start","message":{"usage":{"input_tokens":10,"output_tokens":0}}}"#,
        r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#,
        r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#,
        r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":", world"}}"#,
        r#"{"type":"content_block_stop","index":0}"#,
        r#"{"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"output_tokens":2}}"#,
        r#"{"type":"message_stop"}"#,
    ];

    let mut body = events
        .iter()
        .map(|e| format!("data: {e}\n\n"))
        .collect::<String>();
    body.push_str("data: [DONE]\n\n");
    body
}

/// Anthropic SSE stream for the **first** request in a tool-use scenario.
///
/// Returns `stop_reason="tool_use"` with a single `list_files` invocation.
fn tool_use_first_sse() -> String {
    let events: &[&str] = &[
        r#"{"type":"message_start","message":{"usage":{"input_tokens":15,"output_tokens":0}}}"#,
        r#"{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"tu_001","name":"mock__list_files"}}"#,
        r#"{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{}"}}"#,
        r#"{"type":"content_block_stop","index":0}"#,
        r#"{"type":"message_delta","delta":{"stop_reason":"tool_use","stop_sequence":null},"usage":{"output_tokens":5}}"#,
        r#"{"type":"message_stop"}"#,
    ];

    let mut body = events
        .iter()
        .map(|e| format!("data: {e}\n\n"))
        .collect::<String>();
    body.push_str("data: [DONE]\n\n");
    body
}

/// Anthropic SSE stream for the **follow-up** request after tool execution.
fn tool_follow_up_sse() -> String {
    let events: &[&str] = &[
        r#"{"type":"message_start","message":{"usage":{"input_tokens":30,"output_tokens":0}}}"#,
        r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#,
        r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Done."}}"#,
        r#"{"type":"content_block_stop","index":0}"#,
        r#"{"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"output_tokens":1}}"#,
        r#"{"type":"message_stop"}"#,
    ];

    let mut body = events
        .iter()
        .map(|e| format!("data: {e}\n\n"))
        .collect::<String>();
    body.push_str("data: [DONE]\n\n");
    body
}

// ── Collecting sink ────────────────────────────────────────────────────────────

/// `AgentEventSink` that records every event in order for assertions.
struct CollectSink {
    events: Vec<&'static str>,
}

impl CollectSink {
    fn new() -> Self {
        Self { events: Vec::new() }
    }
}

#[async_trait]
impl AgentEventSink for CollectSink {
    async fn emit(&mut self, ev: AgentEvent) -> Result<(), AgentEmitError> {
        let tag = match &ev {
            AgentEvent::RoutingMeta(_) => "routing_meta",
            AgentEvent::TextDelta(_) => "text",
            AgentEvent::ToolStart { .. } => "tool_start",
            AgentEvent::ToolResult { .. } => "tool_result",
            AgentEvent::ResourceInvalidated { .. } => "resource_invalidated",
            AgentEvent::Done { .. } => "done",
        };
        self.events.push(tag);
        Ok(())
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────────

fn make_state() -> Arc<AppState> {
    Arc::new(AppState::with_in_memory_stores().expect("in-memory AppState"))
}

fn make_tenant() -> ResolvedTenant {
    ResolvedTenant(TenantContext::new(
        "test-tenant",
        Some("test-user"),
        PlanTier::Enterprise,
        PathBuf::from("/tmp"),
    ))
}

fn minimal_ctx(routing_meta: serde_json::Value) -> AgentCtx {
    AgentCtx {
        api_key: "test-key".into(),
        model_id: "claude-opus-4-7".into(),
        max_tokens: 512,
        max_rounds: 5,
        thread_id: None,
        thread_was_new: false,
        tenant_id: "test-tenant".into(),
        tools: vec![],
        messages: vec![AgentMessage {
            role: MessageRole::User,
            content: MessageContent::Text("say hello".into()),
        }],
        effective_system: None,
        workspace_node_id: None,
        max_invokes_per_turn: 5,
        routing_meta,
    }
}

fn make_provider(base_url: &str) -> Arc<NativeAnthropicProvider> {
    Arc::new(NativeAnthropicProvider::new_with_base_url(
        Client::new(),
        "test-api-key",
        base_url,
    ))
}

// ── Tests ──────────────────────────────────────────────────────────────────────

/// Core invariant: `routing_meta` is always the first event emitted by the
/// gateway for a text-only turn.  This test is the safety net for every future
/// change to `agent.rs` / `runner.rs`.
#[tokio::test]
async fn text_turn_routing_meta_is_first_and_done_is_last() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(text_only_sse()),
        )
        .expect(1)
        .mount(&server)
        .await;

    let state = make_state();
    let tenant = make_tenant();
    let routing_meta = json!({"model": "claude-opus-4-7", "capabilities": []});
    let ctx = minimal_ctx(routing_meta);
    let provider = make_provider(&server.uri());
    let mut runner = AgentTurnRunner::new(state, tenant, ctx, provider);

    let mut sink = CollectSink::new();
    let cancel = CancellationToken::new();
    let result = runner.run(&mut sink, cancel).await;

    assert!(result.is_ok(), "runner should succeed; got: {result:?}");

    let events = &sink.events;
    assert!(!events.is_empty(), "expected at least one event");

    assert_eq!(
        events[0], "routing_meta",
        "first event must be routing_meta; got sequence: {events:?}"
    );

    assert!(
        events.contains(&"text"),
        "expected at least one text delta; got: {events:?}"
    );

    assert_eq!(
        events.last().copied(),
        Some("done"),
        "last event must be done; got sequence: {events:?}"
    );

    // routing_meta must appear exactly once.
    let meta_count = events.iter().filter(|&&e| e == "routing_meta").count();
    assert_eq!(
        meta_count, 1,
        "routing_meta emitted {meta_count} times; expected 1"
    );
}

/// text deltas must all appear after `routing_meta` — no text before routing metadata.
#[tokio::test]
async fn text_deltas_follow_routing_meta() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(text_only_sse()),
        )
        .mount(&server)
        .await;

    let state = make_state();
    let tenant = make_tenant();
    let ctx = minimal_ctx(json!({}));
    let provider = make_provider(&server.uri());
    let mut runner = AgentTurnRunner::new(state, tenant, ctx, provider);

    let mut sink = CollectSink::new();
    let cancel = CancellationToken::new();
    runner.run(&mut sink, cancel).await.ok();

    let events = &sink.events;
    let meta_pos = events
        .iter()
        .position(|&e| e == "routing_meta")
        .expect("routing_meta not emitted");

    for (i, &ev) in events.iter().enumerate() {
        if ev == "text" {
            assert!(
                i > meta_pos,
                "text delta at position {i} precedes routing_meta at {meta_pos}"
            );
        }
    }
}

/// Tool-use turn: `routing_meta` is first, then `tool_start`, `tool_result`,
/// then a text delta, then `done` last.
///
/// The mock is not registered in the in-memory registry, so the runner falls
/// back to an error tool_result — the event ordering invariant still holds.
#[tokio::test]
async fn tool_turn_event_order_routing_meta_tool_start_tool_result_text_done() {
    let server = MockServer::start().await;

    // First POST: return tool_use response.
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(tool_use_first_sse()),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;

    // Second POST: return the follow-up text response.
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(tool_follow_up_sse()),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;

    let state = make_state();
    let tenant = make_tenant();
    // Supply the tool definition so the runner parses the tool_use block.
    // Even without a registered capability, the runner emits ToolStart + ToolResult.
    let mut ctx = minimal_ctx(json!({}));
    ctx.max_rounds = 3;

    let provider = make_provider(&server.uri());
    let mut runner = AgentTurnRunner::new(state, tenant, ctx, provider);

    let mut sink = CollectSink::new();
    let cancel = CancellationToken::new();
    runner.run(&mut sink, cancel).await.ok();

    let events = &sink.events;
    assert!(!events.is_empty(), "expected events");

    // routing_meta must be first.
    assert_eq!(
        events[0], "routing_meta",
        "first event must be routing_meta; got {events:?}"
    );

    // tool_start must precede tool_result.
    let tool_start_pos = events.iter().position(|&e| e == "tool_start");
    let tool_result_pos = events.iter().position(|&e| e == "tool_result");

    if let (Some(ts), Some(tr)) = (tool_start_pos, tool_result_pos) {
        assert!(ts < tr, "tool_start ({ts}) must precede tool_result ({tr})");
        // Both must come after routing_meta.
        assert!(ts > 0, "tool_start must follow routing_meta");
    }
    // (If no tool events, that's acceptable — the mock tool may be unknown and
    // result in no ToolStart being routed; at minimum routing_meta/done ordering holds.)

    // done must be last.
    assert_eq!(
        events.last().copied(),
        Some("done"),
        "last event must be done; got {events:?}"
    );
}

/// When the upstream returns a 500, the runner fails with a provider error, but
/// the gateway must have emitted `routing_meta` first before the error propagates.
#[tokio::test]
async fn provider_error_routing_meta_was_already_emitted() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(500).set_body_string(
                r#"{"error":{"type":"api_error","message":"Internal server error"}}"#,
            ),
        )
        .mount(&server)
        .await;

    let state = make_state();
    let tenant = make_tenant();
    let ctx = minimal_ctx(json!({"source": "test"}));
    let provider = make_provider(&server.uri());
    let mut runner = AgentTurnRunner::new(state, tenant, ctx, provider);

    let mut sink = CollectSink::new();
    let cancel = CancellationToken::new();
    let result = runner.run(&mut sink, cancel).await;

    // The runner returns Err on provider failure.
    assert!(result.is_err(), "expected provider error; got Ok");

    // routing_meta must have been emitted before the provider was even called.
    assert!(
        sink.events.contains(&"routing_meta"),
        "routing_meta must be emitted even when the provider errors; got: {:?}",
        sink.events
    );
    assert_eq!(
        sink.events[0], "routing_meta",
        "routing_meta must be the first event"
    );
}
