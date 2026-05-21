//! Shared LLM chain executor — the stateless core of both `PromptChainCapability`
//! and `DynamicPromptCapability`.
//!
//! Takes a `LlmChainConfig` and a render context (JSON), renders the prompt template,
//! calls the LLM, and returns a JSON value.
//!
//! When `cfg.vision = true` the executor reads `input.image_path` from the context,
//! downloads or reads the file, base64-encodes it, and sends it as vision content
//! alongside the text prompt. All LLM calls go through `LlmRegistry` — never through
//! `rig::providers::*::Client` directly.

use crate::capabilities::executor::resolve_image_path;
use crate::capabilities::manifest::LlmChainConfig;
use crate::context::tenant::TenantContext;
use crate::llm::LlmRegistry;
use crate::llm::types::{LlmRequest, LlmResponse};
use crate::prompt::PromptTemplate;
use base64::{Engine as _, engine::general_purpose};
use rig::OneOrMany;
use rig::completion::Message;
use rig::message::{ImageMediaType, UserContent};
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::{debug, instrument};

/// Execute a prompt chain with the given config, render context, and LLM registry.
///
/// `ctx` should contain `{ "input": <tool_input>, "tenant": { ... } }`.
/// When `cfg.vision = true`, `ctx.input.image_path` must be present.
#[instrument(skip(cfg, ctx, llm, tenant), fields(model = %cfg.model, vision = cfg.vision))]
pub async fn run_chain(
    cfg: &LlmChainConfig,
    ctx: &Value,
    llm: &Arc<LlmRegistry>,
    tenant: Option<&TenantContext>,
) -> anyhow::Result<Value> {
    let prompt = PromptTemplate::new(cfg.prompt_template.clone());
    let user_message = prompt.render(ctx);

    debug!(user_message = %user_message, model = %cfg.model, "run_chain executing");

    // Resolve alias → concrete (provider, model) BEFORE building the request so
    // the upstream API receives the model id, not the alias label.
    let binding = llm
        .resolve_binding(&cfg.model, tenant)
        .map_err(|e| anyhow::anyhow!("run_chain: model resolve failed: {e}"))?;
    let provider = llm
        .resolve(&cfg.model, tenant)
        .map_err(|e| anyhow::anyhow!("run_chain: provider resolve failed: {e}"))?;

    if cfg.vision {
        return run_chain_vision(cfg, ctx, &user_message, llm, tenant).await;
    }

    let mut messages = Vec::new();
    if let Some(sys) = &cfg.system_prompt {
        messages.push(Message::System {
            content: sys.clone(),
        });
    }
    messages.push(Message::User {
        content: OneOrMany::one(UserContent::text(user_message)),
    });

    let req = LlmRequest::builder()
        .model(binding.model.clone())
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

/// Vision variant: reads image_path from ctx, base64-encodes, sends as multimodal content.
async fn run_chain_vision(
    cfg: &LlmChainConfig,
    ctx: &Value,
    user_message: &str,
    llm: &Arc<LlmRegistry>,
    tenant: Option<&TenantContext>,
) -> anyhow::Result<Value> {
    // Extract the image path from the input context.
    let image_path = ctx["input"]["image_path"]
        .as_str()
        .or_else(|| ctx["input"]["document_path"].as_str())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "run_chain_vision: vision=true but input.image_path / input.document_path is missing"
            )
        })?;

    let (temp_path, effective_path) = resolve_image_path(image_path).await?;

    let bytes = std::fs::read(&effective_path).map_err(|e| {
        anyhow::anyhow!(
            "run_chain_vision: cannot read file {:?}: {e}",
            effective_path
        )
    })?;

    // Cleanup temp download after reading.
    if let Some(ref tmp) = temp_path {
        let _ = std::fs::remove_file(tmp);
    }

    let b64 = general_purpose::STANDARD.encode(&bytes);

    // Detect media type from extension for correct MIME tag.
    let media_type = match effective_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => ImageMediaType::JPEG,
        "gif" => ImageMediaType::GIF,
        "webp" => ImageMediaType::WEBP,
        _ => ImageMediaType::PNG, // PDF and unknown → PNG wrapper (Anthropic accepts it)
    };

    // Build a multimodal message: image first, then the text instruction.
    let content = OneOrMany::many(vec![
        UserContent::image_base64(b64, Some(media_type), None),
        UserContent::text(user_message),
    ])
    .map_err(|e| anyhow::anyhow!("run_chain_vision: content build failed: {e}"))?;

    let mut messages = Vec::new();
    if let Some(sys) = &cfg.system_prompt {
        messages.push(Message::System {
            content: sys.clone(),
        });
    }
    messages.push(Message::User { content });

    // Resolve the alias to a concrete (provider, model) binding so the request
    // sent upstream contains the actual model id, not the alias string.
    // (Before this fix, `cfg.model = "smart"` was forwarded literally → 404.)
    let binding = llm
        .resolve_binding(&cfg.model, tenant)
        .map_err(|e| anyhow::anyhow!("run_chain_vision: model resolve failed: {e}"))?;
    let provider = llm
        .resolve(&cfg.model, tenant)
        .map_err(|e| anyhow::anyhow!("run_chain_vision: provider resolve failed: {e}"))?;

    let req = LlmRequest::builder()
        .model(binding.model.clone())
        .messages(messages)
        .max_tokens(cfg.max_tokens)
        .build();

    let LlmResponse { content, .. } = provider
        .complete(req)
        .await
        .map_err(|e| anyhow::anyhow!("run_chain_vision: LLM call failed: {e}"))?;

    let json_text = strip_markdown_fences(&content);
    Ok(serde_json::from_str(json_text).unwrap_or_else(|_| json!({ "result": content })))
}

fn strip_markdown_fences(s: &str) -> &str {
    let s = s.trim();
    let s = s.strip_prefix("```json").unwrap_or(s);
    let s = s.strip_prefix("```").unwrap_or(s);
    let s = s.strip_suffix("```").unwrap_or(s);
    s.trim()
}
