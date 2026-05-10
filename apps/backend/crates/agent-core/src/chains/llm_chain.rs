//! Data-driven LLM chain tool — implements `CapabilityProvider` purely from TOML manifest config.
//!
//! Any capability with `kind = "chain"` and a `[chain]` block in its manifest uses
//! this provider instead of a bespoke Rust implementation.

use crate::chains::executor;
use crate::context::tenant::TenantContext;
use crate::llm::LlmRegistry;
use crate::capabilities::manifest::{LlmChainConfig, ToolManifest};
use crate::capabilities::provider::CapabilityProvider;
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tracing::instrument;

pub struct PromptChainCapability {
    manifest: ToolManifest,
    cfg: LlmChainConfig,
    llm: Arc<LlmRegistry>,
}

impl PromptChainCapability {
    pub fn new(manifest: ToolManifest, llm: Arc<LlmRegistry>) -> anyhow::Result<Self> {
        let cfg = manifest.chain.clone().ok_or_else(|| {
            anyhow::anyhow!(
                "PromptChainCapability: manifest '{}' has no [chain] section",
                manifest.name
            )
        })?;
        Ok(Self { manifest, cfg, llm })
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
        let tenant_view = tenant
            .map(|t| {
                serde_json::json!({
                    "id": &*t.tenant_id,
                    "plan": t.plan.to_string(),
                })
            })
            .unwrap_or(Value::Null);
        let ctx = serde_json::json!({ "input": input, "tenant": tenant_view });
        executor::run_chain(&self.cfg, &ctx, &self.llm, tenant).await
    }
}
