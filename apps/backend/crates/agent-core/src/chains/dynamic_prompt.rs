//! Manifest-backed dynamic prompt capability.
//!
//! Reads its `LlmChainConfig` from the capability manifest's `chain` field.
//! Versioning and DB storage have been removed; prompts are managed via
//! capability.toml (or in-memory config for programmatic use).

use crate::capabilities::manifest::{LlmChainConfig, ToolManifest};
use crate::capabilities::provider::CapabilityProvider;
use crate::chains::executor;
use crate::context::tenant::TenantContext;
use crate::llm::LlmRegistry;
use async_trait::async_trait;
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::instrument;

pub struct DynamicPromptCapability {
    manifest: ToolManifest,
    llm: Arc<LlmRegistry>,
    /// Optional override config; if `None`, falls back to `manifest.chain`.
    override_cfg: Option<Arc<LlmChainConfig>>,
}

impl DynamicPromptCapability {
    pub fn new(manifest: ToolManifest, llm: Arc<LlmRegistry>) -> Self {
        Self {
            manifest,
            llm,
            override_cfg: None,
        }
    }

    pub fn with_config(mut self, cfg: LlmChainConfig) -> Self {
        self.override_cfg = Some(Arc::new(cfg));
        self
    }

    fn chain_config(&self) -> anyhow::Result<Arc<LlmChainConfig>> {
        if let Some(cfg) = &self.override_cfg {
            return Ok(Arc::clone(cfg));
        }
        self.manifest
            .chain
            .as_ref()
            .map(|c| Arc::new(c.clone()))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "DynamicPromptCapability '{}' has no chain config in manifest",
                    self.manifest.name
                )
            })
    }
}

#[async_trait]
impl CapabilityProvider for DynamicPromptCapability {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    #[instrument(skip(self, input, tenant), fields(
        tool = %tool_name,
        capability = %self.manifest.name,
    ))]
    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        let cfg = self.chain_config()?;
        let tenant_view = tenant
            .map(|t| json!({ "id": &*t.tenant_id, "plan": t.plan.to_string() }))
            .unwrap_or(Value::Null);
        let ctx = json!({ "input": input, "tenant": tenant_view });
        executor::run_chain(&cfg, &ctx, &self.llm, tenant).await
    }
}
