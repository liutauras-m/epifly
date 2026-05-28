//! `ProjectionRedactor` — mandatory redaction for thread → workspace-node Markdown.
//!
//! Every code path that writes a thread MD body **must** go through this trait.
//! The `#[must_use]` token type `RedactedBody` is passed from `render()` to `write()`
//! so callers cannot forget. Bypassing requires `RedactedBody::unsafe_unredacted()`
//! which exists only for tests.
//!
//! ## Default policy (v1 — conservative)
//! - Tool args: render only capability name + synthesized summary. Raw JSON stays in redb.
//! - Tool results: `"ok"` / `"failed"` + optional one-line summary. Never verbatim.
//! - User text: passed through the PII sieve (email, phone, card, JWT, API key, AWS key, IBAN).
//! - Toggle per tenant: `include_tool_details: bool` (default false). Even when `true`,
//!   known-secret shapes are still stripped.

use regex::Regex;
use serde_json::Value;
use std::borrow::Cow;
use std::sync::OnceLock;

// ── Token type ────────────────────────────────────────────────────────────────

/// Opaque wrapper around a redacted Markdown body.
///
/// Constructed only via `ProjectionRedactor::render`. Use `.into_string()` to get the body.
#[must_use]
pub struct RedactedBody(String);

impl RedactedBody {
    pub fn into_string(self) -> String {
        self.0
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Construct without redaction — **test-only**.
    #[cfg(test)]
    pub fn unsafe_unredacted(body: impl Into<String>) -> Self {
        Self(body.into())
    }
}

// ── Trait ─────────────────────────────────────────────────────────────────────

pub trait ProjectionRedactor: Send + Sync {
    /// Synthesize a safe rendering of a tool invocation argument block.
    /// Default: drops JSON, returns `"[Used: {capability}]"`.
    fn redact_tool_args(&self, capability: &str, _args: &Value) -> String {
        format!("[Used: {capability}]")
    }

    /// Synthesize a safe rendering of a tool result.
    /// Default: `"ok"` / `"failed"` + optional caller-supplied summary line.
    fn redact_tool_result(&self, _capability: &str, result: &Value) -> String {
        let summary = result.get("summary").and_then(|v| v.as_str()).unwrap_or("");
        if result.get("error").is_some() {
            if summary.is_empty() {
                "failed".to_owned()
            } else {
                format!("failed: {summary}")
            }
        } else if summary.is_empty() {
            "ok".to_owned()
        } else {
            format!("ok: {summary}")
        }
    }

    /// Pass user text through the PII sieve.
    fn redact_user_text<'a>(&self, text: &'a str) -> Cow<'a, str>;

    /// Render a slice of `(role, text)` pairs into a `RedactedBody`.
    fn render(&self, messages: &[RenderedMessage]) -> RedactedBody {
        let mut parts = Vec::with_capacity(messages.len());
        for msg in messages {
            let header = match msg.role.as_str() {
                "user" => "**You**",
                "assistant" => "**Assistant**",
                _ => &msg.role,
            };
            let body = match &msg.kind {
                MessageKind::Text(t) => self.redact_user_text(t).into_owned(),
                MessageKind::ToolCall { capability, args } => {
                    self.redact_tool_args(capability, args)
                }
                MessageKind::ToolResult { capability, result } => {
                    self.redact_tool_result(capability, result)
                }
            };
            parts.push(format!("{header}\n\n{body}"));
        }
        RedactedBody(parts.join("\n\n---\n\n"))
    }
}

/// A flattened message ready for projection rendering.
pub struct RenderedMessage {
    pub role: String,
    pub kind: MessageKind,
}

pub enum MessageKind {
    Text(String),
    ToolCall { capability: String, args: Value },
    ToolResult { capability: String, result: Value },
}

// ── Default implementation ────────────────────────────────────────────────────

/// The default v1 redactor. Conservative: strips known-secret shapes always;
/// suppresses raw tool args/results always. `include_tool_details` adds one
/// capability-name line but never raw JSON.
pub struct DefaultProjectionRedactor {
    pub include_tool_details: bool,
}

impl DefaultProjectionRedactor {
    pub fn new() -> Self {
        Self {
            include_tool_details: false,
        }
    }

    pub fn with_tool_details(mut self, include: bool) -> Self {
        self.include_tool_details = include;
        self
    }

    fn pii_patterns() -> &'static Vec<(&'static str, Regex)> {
        static PATTERNS: OnceLock<Vec<(&'static str, Regex)>> = OnceLock::new();
        PATTERNS.get_or_init(|| {
            vec![
                (
                    "email",
                    Regex::new(r"[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}").unwrap(),
                ),
                (
                    "phone",
                    Regex::new(r"\b(\+?1[-.\s]?)?\(?\d{3}\)?[-.\s]?\d{3}[-.\s]?\d{4}\b").unwrap(),
                ),
                ("card", Regex::new(r"\b(?:\d[ -]?){13,19}\b").unwrap()),
                (
                    "jwt",
                    Regex::new(r"ey[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}")
                        .unwrap(),
                ),
                (
                    "api_key",
                    Regex::new(r"(?:Bearer\s+|sk-|pk-|rk-|AKIA)[A-Za-z0-9_\-]{16,}").unwrap(),
                ),
                (
                    "aws_key",
                    Regex::new(r"\b(AKIA|ASIA|AIPA|AROA|ANPA)[A-Z0-9]{16}\b").unwrap(),
                ),
                (
                    "iban",
                    Regex::new(r"\b[A-Z]{2}\d{2}[A-Z0-9]{4}\d{7}(?:[A-Z0-9]{0,16})?\b").unwrap(),
                ),
            ]
        })
    }

    fn scrub_secrets(text: &str) -> Cow<'_, str> {
        let mut result = Cow::Borrowed(text);
        for (label, re) in Self::pii_patterns() {
            let replaced = re.replace_all(result.as_ref(), format!("[REDACTED_{label}]").as_str());
            if let Cow::Owned(s) = replaced {
                result = Cow::Owned(s);
            }
        }
        result
    }
}

impl Default for DefaultProjectionRedactor {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectionRedactor for DefaultProjectionRedactor {
    fn redact_tool_args(&self, capability: &str, _args: &Value) -> String {
        if self.include_tool_details {
            format!("[Used: {capability}]")
        } else {
            String::new()
        }
    }

    fn redact_tool_result(&self, capability: &str, result: &Value) -> String {
        let summary = result.get("summary").and_then(|v| v.as_str()).unwrap_or("");
        if self.include_tool_details {
            let status = if result.get("error").is_some() {
                "failed"
            } else {
                "ok"
            };
            if summary.is_empty() {
                format!("[{capability}: {status}]")
            } else {
                // Still strip secrets from the summary line.
                let clean = Self::scrub_secrets(summary);
                format!("[{capability}: {status}: {clean}]")
            }
        } else {
            String::new()
        }
    }

    fn redact_user_text<'a>(&self, text: &'a str) -> Cow<'a, str> {
        Self::scrub_secrets(text)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn redactor() -> DefaultProjectionRedactor {
        DefaultProjectionRedactor::new()
    }

    #[test]
    fn pii_email_redacted() {
        let r = redactor();
        let out = r.redact_user_text("contact me at alice@example.com please");
        assert!(
            !out.contains("alice@example.com"),
            "email must be redacted: {out}"
        );
        assert!(out.contains("[REDACTED_email]"));
    }

    #[test]
    fn pii_api_key_redacted() {
        let r = redactor();
        let out = r.redact_user_text("use sk-abcdefghijklmnopqrstuvwxyz1234567890 as the key");
        assert!(!out.contains("sk-abc"), "api key must be redacted");
    }

    #[test]
    fn pii_jwt_redacted() {
        let r = redactor();
        let jwt = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
        let input = format!("my token: {jwt}");
        let out = r.redact_user_text(&input);
        assert!(!out.contains("eyJhbGci"), "JWT must be redacted");
    }

    #[test]
    fn tool_args_suppressed_by_default() {
        let r = redactor();
        let result = r.redact_tool_args("search", &json!({"q": "secret"}));
        assert!(result.is_empty(), "tool args should be empty by default");
    }

    #[test]
    fn tool_args_shown_with_include_details() {
        let r = DefaultProjectionRedactor::new().with_tool_details(true);
        let result = r.redact_tool_args("search", &json!({"q": "secret"}));
        assert!(result.contains("search"), "capability name should appear");
        assert!(!result.contains("secret"), "raw json must not appear");
    }

    #[test]
    fn render_produces_must_use_token() {
        let r = DefaultProjectionRedactor::new();
        let msgs = vec![RenderedMessage {
            role: "user".into(),
            kind: MessageKind::Text("hello world".into()),
        }];
        let body = r.render(&msgs);
        assert!(body.as_str().contains("hello world"));
    }

    #[test]
    fn secret_in_tool_result_summary_stripped() {
        let r = DefaultProjectionRedactor::new().with_tool_details(true);
        let result = r.redact_tool_result(
            "fetch",
            &json!({"summary": "fetched token sk-abcdefghijklmnopqrstuvwxyz1234567890 ok"}),
        );
        assert!(
            !result.contains("sk-abc"),
            "secret in summary must be stripped"
        );
    }
}
