//! Native Anthropic HTTP + SSE provider — Step 2.7.
//!
//! All Anthropic wire encoding and SSE parsing lives here.
//! Route handlers and the runner never reference `reqwest` directly.

use super::{
    AgentProvider, ProviderError, ProviderEvent, ProviderEventSink, ProviderRequest,
    ProviderResponse,
};
use agent_core::{AgentMessage, ContentBlock, MessageContent};
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::{Client, Response as ReqwestResponse};
use serde_json::{Value, json};
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::warn;
use uuid::Uuid;

// ── Configuration ─────────────────────────────────────────────────────────────

const MAX_UPSTREAM_ATTEMPTS: u32 = 3;

fn is_retryable_status(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::REQUEST_TIMEOUT
        || status == reqwest::StatusCode::TOO_MANY_REQUESTS
        || status.is_server_error()
}

fn retry_after_header(response: &ReqwestResponse) -> Option<Duration> {
    response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .map(Duration::from_secs)
}

async fn sleep_before_retry(attempt: u32, hint: Option<Duration>) {
    let delay = hint.unwrap_or_else(|| {
        let base_ms = 150_u64.saturating_mul(2_u64.saturating_pow(attempt.saturating_sub(1)));
        let jitter_ms = rand::random::<u64>() % 75;
        Duration::from_millis(base_ms + jitter_ms)
    });
    tokio::time::sleep(delay).await;
}

// ── Metric helpers ────────────────────────────────────────────────────────────

fn record_retry(model: &str, status: impl Into<String>) {
    common::metrics::llm_upstream_retry_count().add(
        1,
        &[
            common::metrics::kv("provider", "anthropic"),
            common::metrics::kv("model", model),
            common::metrics::kv("status", status),
        ],
    );
}

fn record_timeout(model: &str) {
    common::metrics::llm_upstream_timeout_count().add(
        1,
        &[
            common::metrics::kv("provider", "anthropic"),
            common::metrics::kv("model", model),
        ],
    );
}

fn record_retry_exhausted(model: &str) {
    common::metrics::llm_upstream_retry_exhausted_count().add(
        1,
        &[
            common::metrics::kv("provider", "anthropic"),
            common::metrics::kv("model", model),
        ],
    );
}

// ── Typed → Anthropic JSON conversion ────────────────────────────────────────

/// Convert typed `AgentMessage`s to the Anthropic `messages` array JSON.
pub fn messages_to_anthropic_json(messages: &[AgentMessage]) -> Vec<Value> {
    messages
        .iter()
        .map(|m| {
            let role = m.role.as_str();
            match &m.content {
                MessageContent::Text(text) => json!({"role": role, "content": text}),
                MessageContent::Blocks(blocks) => {
                    let content: Vec<Value> = blocks.iter().map(block_to_json).collect();
                    json!({"role": role, "content": content})
                }
            }
        })
        .collect()
}

fn block_to_json(block: &ContentBlock) -> Value {
    match block {
        ContentBlock::Text { text } => json!({"type": "text", "text": text}),
        ContentBlock::ToolUse { id, name, input } => json!({
            "type": "tool_use", "id": id, "name": name, "input": input
        }),
        ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => json!({
            "type": "tool_result",
            "tool_use_id": tool_use_id,
            "content": content,
            "is_error": is_error,
        }),
        ContentBlock::Raw(v) => v.clone(),
    }
}

// ── NativeAnthropicProvider ────────────────────────────────────────────────────

/// Sends requests to the Anthropic Messages API using the shared HTTP client.
///
/// Retry rules (per Step 1.3):
/// - Only retry before any response bytes are received.
/// - Never retry after the first upstream SSE event.
/// - Retries: 408, 429, 5xx; honour `Retry-After`; max 2 retries (3 total).
pub struct NativeAnthropicProvider {
    http: Client,
    api_key: String,
    /// Base URL resolved once at construction; defaults to `ANTHROPIC_API_BASE_URL` env var
    /// or `https://api.anthropic.com`. Stored (not read lazily) so tests can pass in the
    /// wiremock server URL without environment-variable races.
    base_url: String,
}

impl NativeAnthropicProvider {
    pub fn new(http: Client, api_key: String) -> Self {
        let base_url = std::env::var("ANTHROPIC_API_BASE_URL")
            .unwrap_or_else(|_| "https://api.anthropic.com".to_string());
        Self {
            http,
            api_key,
            base_url,
        }
    }

    /// Construct with an explicit base URL — used by tests to point at a wiremock server.
    pub fn new_with_base_url(
        http: Client,
        api_key: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Self {
        Self {
            http,
            api_key: api_key.into(),
            base_url: base_url.into(),
        }
    }

    fn messages_url(&self) -> String {
        format!("{}/v1/messages", self.base_url.trim_end_matches('/'))
    }
}

// ── Shared retry helper ───────────────────────────────────────────────────────

impl NativeAnthropicProvider {
    async fn send_with_retry(
        &self,
        body: &Value,
        stream: bool,
        model: &str,
        request_id: Uuid,
    ) -> Result<ReqwestResponse, ProviderError> {
        for attempt in 1..=MAX_UPSTREAM_ATTEMPTS {
            let response = self
                .http
                .post(self.messages_url())
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .json(body)
                .send()
                .await;

            match response {
                Ok(resp) => {
                    let status = resp.status();
                    if !is_retryable_status(status) || attempt == MAX_UPSTREAM_ATTEMPTS {
                        if is_retryable_status(status) {
                            record_retry_exhausted(model);
                        }
                        return Ok(resp);
                    }
                    record_retry(model, status.as_u16().to_string());
                    warn!(
                        provider = "anthropic",
                        model,
                        %request_id,
                        attempt,
                        status = status.as_u16(),
                        stream,
                        "retrying upstream LLM call after retryable status"
                    );
                    sleep_before_retry(attempt, retry_after_header(&resp)).await;
                }
                Err(e) => {
                    if e.is_timeout() {
                        record_timeout(model);
                    }
                    if attempt == MAX_UPSTREAM_ATTEMPTS {
                        record_retry_exhausted(model);
                        return Err(ProviderError::Transport(format!(
                            "Anthropic request failed: {e}"
                        )));
                    }
                    record_retry(model, "transport");
                    warn!(
                        provider = "anthropic",
                        model,
                        %request_id,
                        attempt,
                        error = %e,
                        stream,
                        "retrying upstream LLM call after transport error"
                    );
                    sleep_before_retry(attempt, None).await;
                }
            }
        }
        Err(ProviderError::Transport(
            "upstream retry loop exited unexpectedly".into(),
        ))
    }
}

// ── AgentProvider impl ────────────────────────────────────────────────────────

#[async_trait]
impl AgentProvider for NativeAnthropicProvider {
    async fn complete(
        &self,
        req: ProviderRequest,
        request_id: Uuid,
    ) -> Result<ProviderResponse, ProviderError> {
        let messages_json = messages_to_anthropic_json(&req.messages);
        let mut body = json!({
            "model": req.model,
            "max_tokens": req.max_tokens,
            "messages": messages_json,
            "tools": req.tools,
        });
        if let Some(sys) = req.system {
            body["system"] = json!(sys);
        }

        let resp = self
            .send_with_retry(&body, false, &req.model, request_id)
            .await?;

        let parsed: Value = resp
            .json()
            .await
            .map_err(|e| ProviderError::Parse(format!("response parse failed: {e}")))?;

        if let Some(err) = parsed.get("error") {
            return Err(ProviderError::UpstreamHttp {
                status: 0,
                body: err["message"].as_str().unwrap_or("unknown").to_string(),
            });
        }

        Ok(ProviderResponse {
            content: parsed["content"].as_array().cloned().unwrap_or_default(),
            stop_reason: parsed["stop_reason"]
                .as_str()
                .unwrap_or("end_turn")
                .to_string(),
            input_tokens: parsed["usage"]["input_tokens"].as_u64().unwrap_or(0),
            output_tokens: parsed["usage"]["output_tokens"].as_u64().unwrap_or(0),
        })
    }

    async fn stream_events(
        &self,
        req: ProviderRequest,
        sink: &mut dyn ProviderEventSink,
        cancel: CancellationToken,
        request_id: Uuid,
    ) -> Result<(), ProviderError> {
        let messages_json = messages_to_anthropic_json(&req.messages);
        let mut body = json!({
            "model": req.model,
            "max_tokens": req.max_tokens,
            "messages": messages_json,
            "tools": req.tools,
            "stream": true,
        });
        if let Some(sys) = req.system {
            body["system"] = json!(sys);
        }

        let resp = self
            .send_with_retry(&body, true, &req.model, request_id)
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let raw = resp.text().await.unwrap_or_default();
            let body_msg = serde_json::from_str::<Value>(&raw)
                .ok()
                .and_then(|v| {
                    v.get("error")
                        .and_then(|e| e.get("message"))
                        .and_then(|m| m.as_str())
                        .map(|s| s.to_string())
                })
                .unwrap_or(raw);
            return Err(ProviderError::UpstreamHttp {
                status,
                body: body_msg,
            });
        }

        let mut byte_stream = resp.bytes_stream();
        let mut buf = String::new();

        while let Some(chunk_result) = byte_stream.next().await {
            if cancel.is_cancelled() {
                break;
            }

            let Ok(bytes) = chunk_result else { break };
            buf.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(pos) = buf.find("\n\n") {
                let block = buf[..pos].to_string();
                buf = buf[pos + 2..].to_string();

                for line in block.lines() {
                    let Some(data) = line.strip_prefix("data: ") else {
                        continue;
                    };
                    if data == "[DONE]" {
                        sink.on_event(ProviderEvent::Done).await?;
                        return Ok(());
                    }
                    let Ok(ev) = serde_json::from_str::<Value>(data) else {
                        continue;
                    };

                    let provider_ev = match ev["type"].as_str().unwrap_or("") {
                        "message_start" => ProviderEvent::InputUsage {
                            input_tokens: ev["message"]["usage"]["input_tokens"]
                                .as_u64()
                                .unwrap_or(0),
                        },
                        "content_block_start" => {
                            let idx = ev["index"].as_u64().unwrap_or(0) as usize;
                            let cb = &ev["content_block"];
                            match cb["type"].as_str().unwrap_or("") {
                                "tool_use" => ProviderEvent::ToolStart {
                                    index: idx,
                                    id: cb["id"].as_str().unwrap_or("").to_string(),
                                    name: cb["name"].as_str().unwrap_or("").to_string(),
                                },
                                _ => continue,
                            }
                        }
                        "content_block_delta" => {
                            let idx = ev["index"].as_u64().unwrap_or(0) as usize;
                            let delta = &ev["delta"];
                            match delta["type"].as_str().unwrap_or("") {
                                "text_delta" => ProviderEvent::TextDelta(
                                    delta["text"].as_str().unwrap_or("").to_string(),
                                ),
                                "input_json_delta" => ProviderEvent::ToolInputDelta {
                                    index: idx,
                                    partial_json: delta["partial_json"]
                                        .as_str()
                                        .unwrap_or("")
                                        .to_string(),
                                },
                                _ => continue,
                            }
                        }
                        "content_block_stop" => ProviderEvent::ContentBlockStop(
                            ev["index"].as_u64().unwrap_or(0) as usize,
                        ),
                        "message_delta" => ProviderEvent::MessageDelta {
                            output_tokens: ev["usage"]["output_tokens"].as_u64().unwrap_or(0),
                            stop_reason: ev["delta"]["stop_reason"]
                                .as_str()
                                .unwrap_or("end_turn")
                                .to_string(),
                        },
                        _ => continue,
                    };
                    sink.on_event(provider_ev).await?;
                }
            }
        }

        sink.on_event(ProviderEvent::Done).await?;
        Ok(())
    }
}
