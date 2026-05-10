use crate::capabilities::card::CapabilityCard;
use crate::capabilities::manifest::{ToolKind, ToolManifest};
use crate::capabilities::mcp_adapter::McpAdapter;
use crate::capabilities::provider::{CapabilityFactory, CapabilityProvider};
use crate::context::tenant::TenantContext;
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

pub struct McpProvider {
    manifest: ToolManifest,
    endpoint: String,
}

impl McpProvider {
    pub fn new(card: CapabilityCard) -> anyhow::Result<Self> {
        let endpoint = card.manifest.config["endpoint"]
            .as_str()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "MCP tool '{}' has no config.endpoint — \
                    add `endpoint: http://...` to its capability.toml config section",
                    card.manifest.name
                )
            })?
            .to_string();
        Ok(Self {
            manifest: card.manifest,
            endpoint,
        })
    }
}

#[async_trait]
impl CapabilityProvider for McpProvider {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        _tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        let adapter = McpAdapter::new(&self.endpoint).map_err(|e| anyhow::anyhow!("{e}"))?;
        adapter
            .call_tool(tool_name, input.clone())
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))
    }
}

/// Factory for `ToolKind::Mcp`.
pub struct McpFactory;

impl CapabilityFactory for McpFactory {
    fn supports(&self, kind: &ToolKind, _name: &str) -> bool {
        matches!(kind, ToolKind::Mcp)
    }

    fn create(&self, card: CapabilityCard) -> anyhow::Result<Arc<dyn CapabilityProvider>> {
        Ok(Arc::new(McpProvider::new(card)?))
    }
}
