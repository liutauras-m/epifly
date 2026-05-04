use crate::llm::error::LlmError;
use bon::Builder;
use common::types::TenantId;
use futures::Stream;
use rig::completion::Message;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

// ── Request / Response ────────────────────────────────────────────────────────

/// A provider-agnostic completion request.
#[derive(Debug, Clone, Builder)]
pub struct LlmRequest {
    /// Alias (e.g. `"opus"`) or concrete model id (e.g. `"claude-opus-4-7"`).
    pub model: String,
    pub messages: Vec<Message>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    /// Anthropic-format tool definitions passed through to the provider.
    #[builder(default)]
    pub tools: Vec<serde_json::Value>,
    /// Tenant making the request — used for telemetry and registry resolution.
    pub tenant: Option<TenantId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub content: String,
    pub usage: Option<LlmUsage>,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

// ── Streaming ─────────────────────────────────────────────────────────────────

/// A single chunk from a streaming response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmChunk {
    pub delta: String,
    pub finish_reason: Option<String>,
}

/// A boxed async stream of chunks, yielded by `LlmProvider::stream`.
pub type LlmStream = Pin<Box<dyn Stream<Item = Result<LlmChunk, LlmError>> + Send>>;

// ── Registry binding ──────────────────────────────────────────────────────────

/// Resolved pair of (provider name, concrete model id) stored in the registry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LlmBinding {
    /// Must match a key in `LlmRegistry::providers`.
    pub provider: String,
    /// Concrete model id passed to the provider (e.g. `"claude-opus-4-7"`).
    pub model: String,
}
