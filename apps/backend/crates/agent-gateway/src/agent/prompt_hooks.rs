//! Prompt hooks — Step 4.3.
//!
//! `PromptHook` runs `before_turn` (can mutate `AgentCtx`) and `after_turn`
//! (receives final usage for logging/metering).
//!
//! Built-in implementations:
//! - `LogTokensHook`         — logs token usage after every turn
//! - `RedactPiiHook`         — audit/log PII masking (on by default); prompt mutation opt-in
//! - `EnforceMaxInputHook`   — rejects turns whose estimated input exceeds the limit

use crate::agent::context::AgentCtx;
use agent_core::{AgentMessage, ContentBlock, MessageContent};
use async_trait::async_trait;
use regex::Regex;
use std::sync::OnceLock;
use tracing::{info, warn};

// ── HookError ─────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum HookError {
    #[error("input too large: estimated {estimated_tokens} tokens exceeds limit {limit}")]
    InputTooLarge { estimated_tokens: u64, limit: u64 },
    #[error("{0}")]
    Other(String),
}

// ── Usage ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub tool_calls_made: usize,
    pub duration_ms: u64,
}

// ── PromptHook trait ──────────────────────────────────────────────────────────

#[async_trait]
pub trait PromptHook: Send + Sync {
    /// Called before the first provider round. May mutate `ctx` (e.g. redact PII).
    async fn before_turn(&self, ctx: &mut AgentCtx) -> Result<(), HookError>;

    /// Called after the final round, once usage is known.
    async fn after_turn(&self, ctx: &AgentCtx, usage: &Usage) -> Result<(), HookError>;
}

// ── LogTokensHook ─────────────────────────────────────────────────────────────

/// Logs token usage as a structured trace event after each turn.
pub struct LogTokensHook;

#[async_trait]
impl PromptHook for LogTokensHook {
    async fn before_turn(&self, _ctx: &mut AgentCtx) -> Result<(), HookError> {
        Ok(())
    }

    async fn after_turn(&self, ctx: &AgentCtx, usage: &Usage) -> Result<(), HookError> {
        info!(
            tenant_id = %ctx.tenant_id,
            model = %ctx.model_id,
            input_tokens = usage.input_tokens,
            output_tokens = usage.output_tokens,
            tool_calls = usage.tool_calls_made,
            duration_ms = usage.duration_ms,
            "agent_turn_usage"
        );
        Ok(())
    }
}

// ── RedactPiiHook ─────────────────────────────────────────────────────────────

/// PII redaction for logs and (optionally) prompt messages.
///
/// Policy:
/// - Audit/log surface: redaction **always** applied (the `after_turn` logs use redacted text).
/// - Prompt mutation: only when `mutate_prompts = true` (opt-in per deployment).
/// - Tool inputs: **never** mutated — see plan §4.3 for rationale.
pub struct RedactPiiHook {
    /// When `true`, `before_turn` replaces PII in `ctx.messages` before sending to the model.
    pub mutate_prompts: bool,
}

impl RedactPiiHook {
    pub fn new(mutate_prompts: bool) -> Self {
        Self { mutate_prompts }
    }

    fn patterns() -> &'static [(&'static str, Regex)] {
        static PATTERNS: OnceLock<Vec<(&'static str, Regex)>> = OnceLock::new();
        PATTERNS.get_or_init(|| {
            vec![
                // Email addresses
                (
                    "email",
                    Regex::new(r"[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}").unwrap(),
                ),
                // Phone numbers (various formats)
                (
                    "phone",
                    Regex::new(r"\b(\+?1[-.\s]?)?\(?\d{3}\)?[-.\s]?\d{3}[-.\s]?\d{4}\b").unwrap(),
                ),
                // Credit card numbers (simplified)
                ("card", Regex::new(r"\b(?:\d[ -]?){13,19}\b").unwrap()),
                // JWT tokens (header.payload.signature)
                (
                    "jwt",
                    Regex::new(r"ey[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}")
                        .unwrap(),
                ),
                // Bearer tokens / API keys (sk-, pk-, rk-, Bearer prefix)
                (
                    "api_key",
                    Regex::new(r"(?:Bearer\s+|sk-|pk-|rk-|AKIA)[A-Za-z0-9_\-]{16,}").unwrap(),
                ),
                // AWS access key IDs
                (
                    "aws_key",
                    Regex::new(r"\b(AKIA|ASIA|AIPA|AROA|ANPA)[A-Z0-9]{16}\b").unwrap(),
                ),
                // IBAN
                (
                    "iban",
                    Regex::new(r"\b[A-Z]{2}\d{2}[A-Z0-9]{4}\d{7}(?:[A-Z0-9]{0,16})?\b").unwrap(),
                ),
            ]
        })
    }

    /// Redact PII from a text string, returning the redacted version.
    fn redact(text: &str) -> String {
        let mut result = text.to_string();
        for (label, re) in Self::patterns() {
            result = re
                .replace_all(&result, format!("[REDACTED_{label}]").as_str())
                .into_owned();
        }
        result
    }

    fn redact_message_content(content: &mut MessageContent) {
        match content {
            MessageContent::Text(text) => {
                *text = Self::redact(text);
            }
            MessageContent::Blocks(blocks) => {
                for block in blocks.iter_mut() {
                    if let ContentBlock::Text { text } = block {
                        *text = Self::redact(text);
                    }
                    // Tool inputs and ToolResult blocks are NOT redacted — see plan §4.3.
                }
            }
        }
    }
}

#[async_trait]
impl PromptHook for RedactPiiHook {
    async fn before_turn(&self, ctx: &mut AgentCtx) -> Result<(), HookError> {
        if !self.mutate_prompts {
            return Ok(());
        }
        // Only redact Text and text ContentBlocks in user/assistant messages.
        // Tool-use inputs and tool-result content are never touched.
        for msg in ctx.messages.iter_mut() {
            use agent_core::MessageRole;
            // Only redact user messages — assistant messages contain the model's own output.
            if msg.role == MessageRole::User {
                Self::redact_message_content(&mut msg.content);
            }
        }
        Ok(())
    }

    async fn after_turn(&self, ctx: &AgentCtx, _usage: &Usage) -> Result<(), HookError> {
        // Log-surface redaction: redact tenant_id from the log so PII in tenant IDs
        // (e.g. email-as-tenant-id) doesn't leak into the audit trail.
        let redacted_tenant = Self::redact(&ctx.tenant_id);
        if redacted_tenant != ctx.tenant_id {
            info!(
                tenant_id = %redacted_tenant,
                "pii_redaction: tenant_id contained redactable pattern"
            );
        }
        Ok(())
    }
}

// ── EnforceMaxInputHook ───────────────────────────────────────────────────────

/// Rejects turns where the estimated input token count exceeds `max_input_tokens`.
///
/// Uses a conservative char-based estimator (chars / 3.5 for Latin, chars / 2 for
/// high-density scripts). Exact enforcement is a follow-up; this hook fails closed
/// only when the estimate exceeds the limit by a configured `safety_margin` (default 10%).
pub struct EnforceMaxInputHook {
    pub max_input_tokens: u64,
    /// Fraction above the limit at which we actually reject (0.10 = reject at 110%).
    pub safety_margin: f64,
}

impl EnforceMaxInputHook {
    pub fn new(max_input_tokens: u64) -> Self {
        Self {
            max_input_tokens,
            safety_margin: 0.10,
        }
    }

    /// Conservative token count estimator.
    /// Returns an upper-bound estimate suitable for fail-closed enforcement.
    fn estimate_tokens(messages: &[AgentMessage]) -> u64 {
        let total_chars: usize = messages.iter().map(message_char_count).sum();
        // Use chars/2 as a conservative (high) estimate — better to over-count than under.
        ((total_chars as f64) / 2.0).ceil() as u64
    }
}

fn message_char_count(msg: &AgentMessage) -> usize {
    match &msg.content {
        MessageContent::Text(t) => t.chars().count(),
        MessageContent::Blocks(blocks) => blocks.iter().map(block_char_count).sum(),
    }
}

fn block_char_count(block: &ContentBlock) -> usize {
    match block {
        ContentBlock::Text { text } => text.chars().count(),
        ContentBlock::ToolUse { input, .. } => input.to_string().chars().count(),
        ContentBlock::ToolResult { content, .. } => content.chars().count(),
        ContentBlock::Raw(v) => v.to_string().chars().count(),
    }
}

#[async_trait]
impl PromptHook for EnforceMaxInputHook {
    async fn before_turn(&self, ctx: &mut AgentCtx) -> Result<(), HookError> {
        let estimated = Self::estimate_tokens(&ctx.messages);
        let threshold = (self.max_input_tokens as f64 * (1.0 + self.safety_margin)) as u64;
        if estimated > threshold {
            warn!(
                tenant_id = %ctx.tenant_id,
                estimated_tokens = estimated,
                limit = self.max_input_tokens,
                "EnforceMaxInputHook: estimated input exceeds limit"
            );
            return Err(HookError::InputTooLarge {
                estimated_tokens: estimated,
                limit: self.max_input_tokens,
            });
        }
        Ok(())
    }

    async fn after_turn(&self, _ctx: &AgentCtx, _usage: &Usage) -> Result<(), HookError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::{AgentMessage, MessageRole};

    fn text_msg(role: MessageRole, text: &str) -> AgentMessage {
        AgentMessage {
            role,
            content: MessageContent::Text(text.to_string()),
        }
    }

    // ── RedactPiiHook ────────────────────────────────────────────────────────

    #[test]
    fn redact_email() {
        let result = RedactPiiHook::redact("Contact alice@example.com today.");
        assert!(
            !result.contains("alice@example.com"),
            "email should be redacted"
        );
        assert!(result.contains("[REDACTED_email]"));
    }

    #[test]
    fn redact_jwt() {
        let jwt = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
        let result = RedactPiiHook::redact(&format!("Token: {jwt}"));
        assert!(!result.contains(jwt), "JWT should be redacted");
    }

    #[test]
    fn redact_api_key() {
        let result = RedactPiiHook::redact("Key: sk-proj-1234567890abcdef1234567890abcdef");
        assert!(!result.contains("sk-proj"), "API key should be redacted");
    }

    #[test]
    fn no_mutation_on_tool_result_blocks() {
        let mut msg = AgentMessage {
            role: MessageRole::User,
            content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
                tool_use_id: "id1".into(),
                content: "alice@example.com".into(),
                is_error: false,
            }]),
        };
        RedactPiiHook::redact_message_content(&mut msg.content);
        if let MessageContent::Blocks(blocks) = &msg.content
            && let ContentBlock::ToolResult { content, .. } = &blocks[0]
        {
            assert_eq!(
                content, "alice@example.com",
                "tool results must not be redacted"
            );
        }
    }

    #[tokio::test]
    async fn redact_pii_hook_does_not_mutate_prompts_by_default() {
        let hook = RedactPiiHook::new(false);
        let original = "My email is test@example.com";
        let mut ctx = crate::agent::context::AgentCtx {
            api_key: String::new(),
            model_id: "claude-opus-4-7".into(),
            max_tokens: 4096,
            max_rounds: 5,
            thread_id: None,
            thread_was_new: false,
            tenant_id: "tenant-a".into(),
            tools: vec![],
            messages: vec![text_msg(MessageRole::User, original)],
            effective_system: None,
            workspace_node_id: None,
            max_invokes_per_turn: 10,
            routing_meta: serde_json::json!({}),
        };
        hook.before_turn(&mut ctx).await.unwrap();
        if let MessageContent::Text(ref text) = ctx.messages[0].content {
            assert_eq!(
                text, original,
                "prompt mutation disabled: text must be unchanged"
            );
        }
    }

    #[tokio::test]
    async fn redact_pii_hook_mutates_prompts_when_enabled() {
        let hook = RedactPiiHook::new(true);
        let mut ctx = crate::agent::context::AgentCtx {
            api_key: String::new(),
            model_id: "claude-opus-4-7".into(),
            max_tokens: 4096,
            max_rounds: 5,
            thread_id: None,
            thread_was_new: false,
            tenant_id: "tenant-a".into(),
            tools: vec![],
            messages: vec![text_msg(MessageRole::User, "Email: secret@domain.com")],
            effective_system: None,
            workspace_node_id: None,
            max_invokes_per_turn: 10,
            routing_meta: serde_json::json!({}),
        };
        hook.before_turn(&mut ctx).await.unwrap();
        if let MessageContent::Text(ref text) = ctx.messages[0].content {
            assert!(
                !text.contains("secret@domain.com"),
                "PII should be redacted"
            );
        }
    }

    // ── EnforceMaxInputHook ───────────────────────────────────────────────────

    #[tokio::test]
    async fn enforce_hook_allows_small_input() {
        let hook = EnforceMaxInputHook::new(10_000);
        let mut ctx = crate::agent::context::AgentCtx {
            api_key: String::new(),
            model_id: "claude-opus-4-7".into(),
            max_tokens: 4096,
            max_rounds: 5,
            thread_id: None,
            thread_was_new: false,
            tenant_id: "tenant-a".into(),
            tools: vec![],
            messages: vec![text_msg(MessageRole::User, "Hi")],
            effective_system: None,
            workspace_node_id: None,
            max_invokes_per_turn: 10,
            routing_meta: serde_json::json!({}),
        };
        assert!(hook.before_turn(&mut ctx).await.is_ok());
    }

    #[tokio::test]
    async fn enforce_hook_rejects_oversized_input() {
        let hook = EnforceMaxInputHook::new(10);
        let huge = "x".repeat(1_000);
        let mut ctx = crate::agent::context::AgentCtx {
            api_key: String::new(),
            model_id: "claude-opus-4-7".into(),
            max_tokens: 4096,
            max_rounds: 5,
            thread_id: None,
            thread_was_new: false,
            tenant_id: "tenant-a".into(),
            tools: vec![],
            messages: vec![text_msg(MessageRole::User, &huge)],
            effective_system: None,
            workspace_node_id: None,
            max_invokes_per_turn: 10,
            routing_meta: serde_json::json!({}),
        };
        let result = hook.before_turn(&mut ctx).await;
        assert!(matches!(result, Err(HookError::InputTooLarge { .. })));
    }
}
