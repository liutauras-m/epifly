//! Data-driven LLM chain tool — implements `CapabilityProvider` purely from TOML manifest config.
//!
//! Any capability with `kind = "chain"` and a `[chain]` block in its manifest uses
//! this provider instead of a bespoke Rust implementation.

use crate::context::tenant::TenantContext;
use crate::llm::types::{LlmRequest, LlmResponse};
use crate::llm::LlmRegistry;
use crate::prompt::PromptTemplate;
use crate::tools::manifest::{LlmChainConfig, ToolManifest};
use crate::tools::provider::CapabilityProvider;
use async_trait::async_trait;
use rig::completion::Message;
use rig::message::UserContent;
use rig::OneOrMany;
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::{debug, instrument};

pub struct PromptChainCapability {
    manifest: ToolManifest,
    cfg: LlmChainConfig,
    prompt: PromptTemplate,
    llm: Arc<LlmRegistry>,
}

impl PromptChainCapability {
    pub fn new(manifest: ToolManifest, llm: Arc<LlmRegistry>) -> anyhow::Result<Self> {
        let cfg = manifest
            .chain
            .clone()
            .ok_or_else(|| anyhow::anyhow!("PromptChainCapability: manifest '{}' has no [chain] section", manifest.name))?;
        let prompt = PromptTemplate::new(cfg.prompt_template.clone());
        Ok(Self { manifest, cfg, prompt, llm })
    }
}

#[async_trait]
impl CapabilityProvider for PromptChainCapability {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    #[instrument(skip(self, input, tenant), fields(tool = %tool_name, capability = %self.manifest.name))]
    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        // Build render context: { input: <input>, tenant: {...} }
        let tenant_view = tenant
            .map(|t| {
                json!({
                    "id": &*t.tenant_id,
                    "plan": t.plan.to_string(),
                })
            })
            .unwrap_or(Value::Null);

        let ctx = json!({ "input": input, "tenant": tenant_view });
        let user_message = self.prompt.render(&ctx);

        debug!(user_message = %user_message, model = %self.cfg.model, "PromptChainCapability invoking");

        // Build LLM request.
        let mut messages = Vec::new();
        if let Some(sys) = &self.cfg.system_prompt {
            messages.push(Message::System { content: sys.clone() });
        }
        messages.push(Message::User {
            content: OneOrMany::one(UserContent::text(user_message)),
        });

        let provider = self
            .llm
            .resolve(&self.cfg.model, tenant)
            .map_err(|e| anyhow::anyhow!("PromptChainCapability: model resolve failed: {e}"))?;

        let req = LlmRequest::builder()
            .model(self.cfg.model.clone())
            .messages(messages)
            .max_tokens(self.cfg.max_tokens)
            .build();

        let LlmResponse { content, .. } = provider
            .complete(req)
            .await
            .map_err(|e| anyhow::anyhow!("PromptChainCapability: LLM call failed: {e}"))?;

        // Try to parse response as JSON; fall back to plain text.
        let output: Value = serde_json::from_str(&content)
            .unwrap_or_else(|_| json!({ "result": content }));

        // Optional output schema validation (best-effort — warn but don't fail).
        if let Some(_schema) = &self.cfg.output_schema {
            // Schema validation can be added here when jsonschema crate is wired in.
            // For now we accept all outputs — the validator in tools/validator.rs
            // enforces schema correctness at registration time.
        }

        Ok(output)
    }
}
