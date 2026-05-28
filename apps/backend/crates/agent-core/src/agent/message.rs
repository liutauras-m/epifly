//! Typed message/content/tool model — Step 2.6.
//!
//! `AgentCtx.messages` uses these types; encoding to Anthropic JSON lives only
//! in `agent-gateway/src/agent/provider/anthropic.rs`.

use serde_json::Value;

// ── MessageRole ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
}

impl MessageRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
        }
    }
}

// ── ContentBlock ──────────────────────────────────────────────────────────────

/// A single content block in a multi-part message.
#[derive(Debug, Clone)]
pub enum ContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
    /// Passthrough block for Anthropic-specific types (image, document, etc.)
    /// whose schema the typed model does not need to understand.
    Raw(Value),
}

// ── MessageContent ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum MessageContent {
    /// Single text string (most common case — avoids a Vec allocation).
    Text(String),
    /// Multi-part content with one or more typed blocks.
    Blocks(Vec<ContentBlock>),
}

// ── AgentMessage ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AgentMessage {
    pub role: MessageRole,
    pub content: MessageContent,
}

impl AgentMessage {
    pub fn user_text(text: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: MessageContent::Text(text.into()),
        }
    }

    pub fn assistant_text(text: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: MessageContent::Text(text.into()),
        }
    }

    pub fn user_blocks(blocks: Vec<ContentBlock>) -> Self {
        Self {
            role: MessageRole::User,
            content: MessageContent::Blocks(blocks),
        }
    }

    pub fn assistant_blocks(blocks: Vec<ContentBlock>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: MessageContent::Blocks(blocks),
        }
    }

    /// Approximate byte count for token-budget estimation.
    pub fn estimated_bytes(&self) -> usize {
        match &self.content {
            MessageContent::Text(t) => t.len(),
            MessageContent::Blocks(blocks) => blocks
                .iter()
                .map(|b| match b {
                    ContentBlock::Text { text } => text.len(),
                    ContentBlock::ToolUse { name, input, .. } => {
                        name.len() + input.to_string().len()
                    }
                    ContentBlock::ToolResult { content, .. } => content.len(),
                    ContentBlock::Raw(v) => v.to_string().len(),
                })
                .sum(),
        }
    }
}
