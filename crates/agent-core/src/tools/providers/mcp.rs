use crate::context::tenant::TenantContext;
use crate::tools::card::ToolCard;
use crate::tools::manifest::ToolManifest;
use crate::tools::mcp_adapter::McpAdapter;
use crate::tools::provider::ToolProvider;
use async_trait::async_trait;
use serde_json::Value;

pub struct McpProvider {
    manifest: ToolManifest,
    endpoint: String,
}

impl McpProvider {
    pub fn new(card: ToolCard) -> anyhow::Result<Self> {
        let endpoint = card.manifest.config["endpoint"]
            .as_str()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "MCP tool '{}' has no config.endpoint — \
                    add `endpoint: http://...` to its capability.yaml config section",
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
impl ToolProvider for McpProvider {
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
