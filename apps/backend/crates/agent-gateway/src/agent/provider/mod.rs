//! Provider abstraction boundary — Step 2.2.
//!
//! Anthropic JSON encoding/decoding lives only in `provider/anthropic.rs`.
//! Route handlers never reference `reqwest` or provider-specific types.

use agent_core::AgentMessage;
use async_trait::async_trait;
use serde_json::Value;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub mod anthropic;

// ── Wire types ────────────────────────────────────────────────────────────────

/// A single provider request (maps to Anthropic messages shape today).
pub struct ProviderRequest {
    pub model: String,
    pub max_tokens: u64,
    /// Full conversation history + new user turn, typed.
    /// Encoding to Anthropic JSON is done in `provider/anthropic.rs`.
    pub messages: Vec<AgentMessage>,
    /// Tool definitions; empty for text-only turns.
    pub tools: Vec<Value>,
    pub system: Option<String>,
}

/// Non-streaming provider response (blocking path).
pub struct ProviderResponse {
    pub content: Vec<Value>,
    pub stop_reason: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

// ── Provider event stream ─────────────────────────────────────────────────────

/// Typed events emitted by the provider during a streaming completion.
///
/// The runner consumes these via `ProviderEventSink` and translates them to
/// typed `AgentEvent`s for the downstream sink.
#[derive(Debug)]
pub enum ProviderEvent {
    /// Input-token usage reported at message_start.
    InputUsage { input_tokens: u64 },
    /// Text delta from the model.
    TextDelta(String),
    /// A new tool_use block has started at `index`.
    ToolStart {
        index: usize,
        id: String,
        name: String,
    },
    /// Partial JSON for the tool_use input at `index`.
    ToolInputDelta { index: usize, partial_json: String },
    /// A content block has ended at `index`.
    ContentBlockStop(usize),
    /// Final usage and stop_reason for the message.
    MessageDelta {
        output_tokens: u64,
        stop_reason: String,
    },
    /// End-of-stream sentinel.
    Done,
}

// ── Provider error ────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("upstream transport error: {0}")]
    Transport(String),
    #[error("upstream returned {status}: {body}")]
    UpstreamHttp { status: u16, body: String },
    #[error("parse error: {0}")]
    Parse(String),
    #[error("configuration error: {0}")]
    Config(String),
}

// ── ProviderEventSink — callback sink during streaming ────────────────────────

/// Receives `ProviderEvent`s as they arrive from the upstream stream.
///
/// The runner implements this to accumulate tool input JSON and pipe
/// text/tool-start events immediately to the `AgentEventSink`.
#[async_trait]
pub trait ProviderEventSink: Send {
    async fn on_event(&mut self, ev: ProviderEvent) -> Result<(), ProviderError>;
}

// ── AgentProvider — the provider trait ───────────────────────────────────────

#[async_trait]
pub trait AgentProvider: Send + Sync {
    /// Non-streaming completion (blocking path).
    async fn complete(
        &self,
        req: ProviderRequest,
        request_id: Uuid,
    ) -> Result<ProviderResponse, ProviderError>;

    /// Streaming completion: emits `ProviderEvent`s through `sink` until done or cancelled.
    async fn stream_events(
        &self,
        req: ProviderRequest,
        sink: &mut dyn ProviderEventSink,
        cancel: CancellationToken,
        request_id: Uuid,
    ) -> Result<(), ProviderError>;
}
