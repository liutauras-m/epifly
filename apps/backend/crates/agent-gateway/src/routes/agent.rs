pub use crate::agent::merge_pinned;
/// POST /v1/agent/completions — Anthropic tool-use agent loop with optional thread memory.
///
/// Supports both blocking (default) and streaming (`"stream": true`) modes.
/// Both paths share `AgentTurnRunner`; route handlers are pure HTTP wiring (~200 lines).
use crate::agent::{
    AgentError, AgentTurnRunner, BlockingSink, NativeAnthropicProvider, SseSink, build_ctx,
};
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
use serde_json::Value;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use tracing::{instrument, warn};
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
        stream_agent(state, tenant, limits, req)
            .await
            .into_response()
    } else {
        match blocking_agent(state, tenant, limits, req).await {
            Ok(v) => Json(v).into_response(),
            Err(e) => e.into_response(),
        }
    }
}

// ── Blocking path ─────────────────────────────────────────────────────────────

async fn blocking_agent(
    state: Arc<AppState>,
    tenant: ResolvedTenant,
    limits: PlanLimits,
    req: crate::routes::chat::ChatRequest,
) -> Result<Value, HttpError> {
    let ctx = build_ctx(&state, &tenant, limits, &req).await?;

    let http = state.http_upstream.clone();
    let provider = Arc::new(NativeAnthropicProvider::new(http, ctx.api_key.clone()));
    let completion_id = format!("chatcmpl-{}", Uuid::new_v4());
    let mut sink = BlockingSink::new(completion_id, ctx.model_id.clone(), ctx.thread_id.clone());
    let cancel = CancellationToken::new();
    let mut runner = AgentTurnRunner::new(Arc::clone(&state), tenant, ctx, provider);

    runner.run(&mut sink, cancel).await.map_err(|e| match e {
        AgentError::MaxRoundsExceeded => {
            map_rig_error("Exceeded max tool call rounds without a final response")
        }
        AgentError::Provider(p) => map_rig_error(p.to_string()),
        AgentError::Config(m) => map_rig_error(m),
        AgentError::ClientGone => map_rig_error("unexpected: ClientGone in blocking path"),
    })?;

    Ok(serde_json::json!({
        "id": sink.completion_id,
        "object": "chat.completion",
        "model": sink.model,
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": sink.text},
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens":     sink.total_input,
            "completion_tokens": sink.total_output,
            "total_tokens":      sink.total_input + sink.total_output,
        },
        "tool_calls_made": sink.tool_calls_made,
        "thread_id": sink.thread_id_after,
    }))
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
                let sink = SseSink::new(
                    tx.clone(),
                    "chatcmpl-init-error".into(),
                    "claude-opus-4-7".into(),
                );
                sink.send_error(&e.body.message, None).await;
                return;
            }
        };

        let thread_id = ctx.thread_id.clone();
        let model = ctx.model_id.clone();
        let completion_id = format!("chatcmpl-{}", Uuid::new_v4());
        let api_key = ctx.api_key.clone();

        let http = state.http_upstream.clone();
        let provider = Arc::new(NativeAnthropicProvider::new(http, api_key));
        let cancel = CancellationToken::new();
        let mut sink = SseSink::new(tx.clone(), completion_id, model.clone());
        let mut runner = AgentTurnRunner::new(Arc::clone(&state), tenant, ctx, provider);

        if let Err(e) = runner.run(&mut sink, cancel).await {
            let msg = match &e {
                AgentError::MaxRoundsExceeded => {
                    "Exceeded maximum tool call rounds without a final response".to_string()
                }
                AgentError::Provider(p) => p.to_string(),
                AgentError::ClientGone => return,
                AgentError::Config(m) => m.clone(),
            };
            sink.send_error(&msg, thread_id.as_deref()).await;
        }
    });

    Sse::new(ReceiverStream::new(rx))
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
    #[test]
    fn merge_pinned_survives_truncation() {
        let pinned = vec![tool("extract_invoice")];
        let semantic = vec![
            tool("save_document"),
            tool("read_file"),
            tool("upload_file"),
        ];
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
        assert_eq!(
            deduped_count, 1,
            "save_document should appear exactly once; got {names:?}"
        );
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

// ── Step 0.2 — SSE upstream mock harness ─────────────────────────────────────
//
// Verifies that `stream_agent` emits events in the correct order:
//   1. `routing_meta` is the first SSE delta (PR 3.B invariant).
//   2. `content` deltas follow.
//   3. The stream terminates with `data: [DONE]`.
#[cfg(test)]
mod sse_harness {
    use super::*;
    use agent_core::{PlanTier, TenantContext};
    use std::path::PathBuf;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    struct EnvGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var(key).ok();
            unsafe { std::env::set_var(key, value) };
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(value) = &self.previous {
                unsafe { std::env::set_var(self.key, value) };
            } else {
                unsafe { std::env::remove_var(self.key) };
            }
        }
    }

    fn dev_tenant() -> ResolvedTenant {
        ResolvedTenant(TenantContext::new(
            "test-tenant",
            None::<&str>,
            PlanTier::Enterprise,
            PathBuf::from("/tmp"),
        ))
    }

    /// Minimal Anthropic SSE body: text response ending with stop_reason=end_turn.
    fn canned_anthropic_sse() -> String {
        [
            r#"data: {"type":"message_start","message":{"usage":{"input_tokens":10,"output_tokens":0}}}"#,
            "",
            r#"data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#,
            "",
            r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello world"}}"#,
            "",
            r#"data: {"type":"content_block_stop","index":0}"#,
            "",
            r#"data: {"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"output_tokens":2}}"#,
            "",
            r#"data: {"type":"message_stop"}"#,
            "",
            "data: [DONE]",
            "",
        ]
        .join("\n")
    }

    /// `routing_meta` must be the first SSE delta, followed by content, then `[DONE]`.
    #[tokio::test]
    async fn routing_meta_is_first_delta_then_content_then_done() {
        let _anthropic_key = EnvGuard::set("ANTHROPIC_API_KEY", "test-key");

        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(canned_anthropic_sse()),
            )
            .mount(&mock_server)
            .await;
        let _anthropic_base_url = EnvGuard::set("ANTHROPIC_API_BASE_URL", &mock_server.uri());

        let state = crate::state::AppState::with_in_memory_stores().expect("in-memory AppState");
        let state = Arc::new(state);

        let tenant = dev_tenant();
        let limits = PlanTier::Enterprise.limits();
        let req = crate::routes::chat::ChatRequest {
            model: None,
            messages: vec![crate::routes::chat::ChatMessage {
                role: "user".into(),
                content: "Hello".into(),
            }],
            max_tokens: None,
            stream: Some(true),
            thread_id: None,
            workspace_node_id: None,
            max_turns: None,
            attachment_content: vec![],
            attachment_ids: vec![],
            forced_capability: None,
        };

        let sse = stream_agent(Arc::clone(&state), tenant, limits, req).await;
        let resp = sse.into_response();
        let bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
            .await
            .expect("collect SSE body");
        let raw = std::str::from_utf8(&bytes).expect("body is utf-8");

        let events: Vec<serde_json::Value> = raw
            .lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .filter(|&d| d != "[DONE]")
            .filter_map(|d| serde_json::from_str(d).ok())
            .collect();

        assert!(!events.is_empty(), "expected SSE events; raw body:\n{raw}");

        // Invariant 1: first event carries routing_meta.
        let first = &events[0];
        assert!(
            first["choices"][0]["delta"].get("routing_meta").is_some(),
            "first delta must carry routing_meta; got:\n{first}"
        );

        // Invariant 2: at least one event carries content.
        let content_idx = events
            .iter()
            .position(|e| e["choices"][0]["delta"].get("content").is_some());
        assert!(
            content_idx.is_some(),
            "expected at least one content delta; events:\n{events:#?}"
        );

        // Invariant 3: routing_meta precedes the first content delta.
        let routing_idx = events
            .iter()
            .position(|e| e["choices"][0]["delta"].get("routing_meta").is_some())
            .unwrap();
        assert!(
            routing_idx < content_idx.unwrap(),
            "routing_meta (idx {routing_idx}) must precede first content delta (idx {})",
            content_idx.unwrap()
        );

        // Invariant 4: stream terminates with [DONE].
        assert!(
            raw.contains("data: [DONE]"),
            "SSE body must contain the [DONE] sentinel"
        );
    }
}
