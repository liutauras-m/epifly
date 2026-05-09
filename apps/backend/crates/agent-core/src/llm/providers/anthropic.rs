use crate::llm::error::LlmError;
use crate::llm::provider::CompletionProvider;
use crate::llm::types::{LlmChunk, LlmRequest, LlmResponse, LlmStream, LlmUsage};
use async_trait::async_trait;
use futures::StreamExt;
use rig::client::ProviderClient;
use rig::client::completion::CompletionClient;
use rig::completion::CompletionModel;
use rig::message::{AssistantContent, Message};
use rig::providers::anthropic;
use rig::streaming::StreamedAssistantContent;
use tracing::instrument;

// ── AnthropicProvider ─────────────────────────────────────────────────────────

pub struct AnthropicProvider {
    client: anthropic::Client,
}

impl AnthropicProvider {
    /// Construct from an already-built Rig Anthropic client.
    pub fn with_client(client: anthropic::Client) -> Self {
        Self { client }
    }

    /// Construct from `ANTHROPIC_API_KEY` environment variable.
    pub fn from_env() -> Result<Self, LlmError> {
        let client = anthropic::Client::from_env()
            .map_err(|e| LlmError::Config(format!("ANTHROPIC_API_KEY: {e}")))?;
        Ok(Self { client })
    }

    /// Extract first text from AssistantContent items.
    fn extract_text(choice: &rig::OneOrMany<AssistantContent>) -> String {
        choice
            .iter()
            .find_map(|c| {
                if let AssistantContent::Text(t) = c {
                    Some(t.text.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default()
    }
}

// ── CompletionProvider impl ──────────────────────────────────────────────────────────

#[async_trait]
impl CompletionProvider for AnthropicProvider {
    fn name(&self) -> &'static str {
        "anthropic"
    }

    fn supports_vision(&self) -> bool {
        true
    }

    #[instrument(
        skip_all,
        fields(
            provider = "anthropic",
            model = %req.model,
            tenant = ?req.tenant,
            streaming = false,
            tool_count = req.tools.len(),
        )
    )]
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError> {
        let model = self.client.completion_model(&req.model);
        // Build request using the model's builder to get correct defaults.
        let mut builder = model.completion_request(Message::User {
            content: rig::OneOrMany::one(rig::message::UserContent::text(
                req.messages
                    .iter()
                    .filter_map(|m| match m {
                        Message::User { content } => {
                            let texts: Vec<_> = content
                                .iter()
                                .filter_map(|c| {
                                    if let rig::message::UserContent::Text(t) = c {
                                        Some(t.text.as_str())
                                    } else {
                                        None
                                    }
                                })
                                .collect();
                            if texts.is_empty() { None } else { Some(texts.join("\n")) }
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            )),
        });
        if let Some(t) = req.temperature {
            builder = builder.temperature(t as f64);
        }
        if let Some(mt) = req.max_tokens {
            builder = builder.max_tokens(mt as u64);
        }
        let rig_req = builder.build();
        let resp = model
            .completion(rig_req)
            .await
            .map_err(LlmError::RigCompletion)?;

        Ok(LlmResponse {
            content: Self::extract_text(&resp.choice),
            usage: Some(LlmUsage {
                input_tokens: resp.usage.input_tokens as u32,
                output_tokens: resp.usage.output_tokens as u32,
            }),
            finish_reason: None,
        })
    }

    /// Uses Rig 0.36 native SSE streaming via `CompletionModel::stream`.
    /// Text deltas are forwarded as `LlmChunk`s; the final chunk carries `finish_reason = "stop"`.
    #[instrument(
        skip_all,
        fields(
            provider = "anthropic",
            model = %req.model,
            tenant = ?req.tenant,
            streaming = true,
            tool_count = req.tools.len(),
        )
    )]
    async fn stream(&self, req: LlmRequest) -> Result<LlmStream, LlmError> {
        let model = self.client.completion_model(&req.model);
        let mut builder = model.completion_request(Message::User {
            content: rig::OneOrMany::one(rig::message::UserContent::text(
                req.messages
                    .iter()
                    .filter_map(|m| match m {
                        Message::User { content } => {
                            let texts: Vec<_> = content
                                .iter()
                                .filter_map(|c| {
                                    if let rig::message::UserContent::Text(t) = c {
                                        Some(t.text.as_str())
                                    } else {
                                        None
                                    }
                                })
                                .collect();
                            if texts.is_empty() { None } else { Some(texts.join("\n")) }
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            )),
        });
        if let Some(t) = req.temperature {
            builder = builder.temperature(t as f64);
        }
        if let Some(mt) = req.max_tokens {
            builder = builder.max_tokens(mt as u64);
        }
        let rig_req = builder.build();

        let rig_stream = model
            .stream(rig_req)
            .await
            .map_err(|e| LlmError::Streaming(e.to_string()))?;

        // Map Rig's StreamedAssistantContent chunks to our LlmChunk type.
        let mapped = rig_stream.filter_map(|item| async move {
            match item {
                Ok(StreamedAssistantContent::Text(t)) => Some(Ok(LlmChunk {
                    delta: t.text.clone(),
                    finish_reason: None,
                })),
                // Final response sentinel — emit finish_reason.
                Ok(StreamedAssistantContent::Final(_)) => Some(Ok(LlmChunk {
                    delta: String::new(),
                    finish_reason: Some("stop".to_string()),
                })),
                // Tool call / reasoning chunks are silently skipped in the streaming text path.
                Ok(_) => None,
                Err(e) => Some(Err(LlmError::Streaming(e.to_string()))),
            }
        });

        Ok(Box::pin(mapped))
    }

}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that `from_env` respects the presence/absence of ANTHROPIC_API_KEY.
    /// Both assertions run in the same test to avoid parallel-test env-var races.
    #[test]
    fn from_env_env_var_handling() {
        let saved = std::env::var("ANTHROPIC_API_KEY").ok();

        // With key set — should succeed (no network call at construction time).
        // SAFETY: test-only mutation; this test runs serially within the process.
        unsafe { std::env::set_var("ANTHROPIC_API_KEY", "test-key-does-not-matter") };
        assert!(
            AnthropicProvider::from_env().is_ok(),
            "from_env should succeed when ANTHROPIC_API_KEY is set"
        );

        // Without key — should fail.
        unsafe { std::env::remove_var("ANTHROPIC_API_KEY") };
        assert!(
            AnthropicProvider::from_env().is_err(),
            "from_env should fail when ANTHROPIC_API_KEY is absent"
        );

        // Restore.
        if let Some(key) = saved {
            unsafe { std::env::set_var("ANTHROPIC_API_KEY", key) };
        }
    }
}
