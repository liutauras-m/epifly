//! Shared LLM chain executor — the stateless core of both `PromptChainCapability`
//! and `DynamicPromptCapability`.
//!
//! Takes a `LlmChainConfig` and a render context (JSON), renders the prompt template,
//! calls the LLM, and returns a JSON value.

use crate::context::tenant::TenantContext;
use crate::llm::LlmRegistry;
use crate::llm::types::{LlmRequest, LlmResponse};
use crate::prompt::PromptTemplate;
use crate::capabilities::manifest::LlmChainConfig;
use rig::OneOrMany;
use rig::completion::Message;
use rig::message::UserContent;
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::debug;

/// Execute a prompt chain with the given config, render context, and LLM registry.
///
/// `ctx` should contain `{ "input": <tool_input>, "tenant": { ... } }`.
pub async fn run_chain(
    cfg: &LlmChainConfig,
    ctx: &Value,
    llm: &Arc<LlmRegistry>,
    tenant: Option<&TenantContext>,
) -> anyhow::Result<Value> {
    let prompt = PromptTemplate::new(cfg.prompt_template.clone());
    let user_message = prompt.render(ctx);

    debug!(user_message = %user_message, model = %cfg.model, "run_chain executing");

    let mut messages = Vec::new();
    if let Some(sys) = &cfg.system_prompt {
        messages.push(Message::System {
            content: sys.clone(),
        });
    }
    messages.push(Message::User {
        content: OneOrMany::one(UserContent::text(user_message)),
    });

    let provider = llm
        .resolve(&cfg.model, tenant)
        .map_err(|e| anyhow::anyhow!("run_chain: model resolve failed: {e}"))?;

    let req = LlmRequest::builder()
        .model(cfg.model.clone())
        .messages(messages)
        .max_tokens(cfg.max_tokens)
        .build();

    let LlmResponse { content, .. } = provider
        .complete(req)
        .await
        .map_err(|e| anyhow::anyhow!("run_chain: LLM call failed: {e}"))?;

    // Try to parse response as JSON; fall back to plain text.
    Ok(serde_json::from_str(&content).unwrap_or_else(|_| json!({ "result": content })))
}
