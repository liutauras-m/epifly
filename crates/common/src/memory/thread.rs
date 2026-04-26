use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A persistent conversation thread (OpenAI Assistants-compatible).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Thread {
    /// ULID — time-sortable, lexicographically ordered.
    pub id: String,
    pub tenant_id: String,
    /// Auto-summarized title, set after the first assistant reply.
    pub title: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
    /// Running count of messages (maintained by the store).
    pub message_count: usize,
    /// LLM-generated summary injected as system context once the thread
    /// exceeds MAX_MESSAGES_BEFORE_SUMMARY.
    pub summary: Option<String>,
    pub metadata: serde_json::Value,
}

/// A single message within a thread.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    /// "user" | "assistant" | "tool"
    pub role: String,
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub timestamp: DateTime<Utc>,
    /// Zero-based position within the thread (used for ordering).
    pub seq: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
    pub output: Option<String>,
}
