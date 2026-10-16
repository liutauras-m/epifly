//! `RemoteMcpCapability` — dynamically-registered MCP provider.
//!
//! Unlike `McpProvider` (file-based, kind=Mcp), this type is constructed
//! entirely from a JSON registration payload and requires no TOML on disk.
//! It is created by `POST /admin/capabilities/register` and stored in
//! `capability_specs` with strategy = "remote_mcp".

use crate::capabilities::manifest::ToolManifest;
use crate::capabilities::mcp_adapter::McpAdapter;
use crate::capabilities::mcp_allowlist::McpAllowlist;
use crate::capabilities::provider::CapabilityProvider;
use crate::context::tenant::TenantContext;
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

pub struct RemoteMcpCapability {
    manifest: ToolManifest,
    endpoint: String,
}

impl RemoteMcpCapability {
    /// Refused unless `endpoint` passes the exact-host MCP allowlist.
    pub fn new(manifest: ToolManifest, endpoint: String) -> anyhow::Result<Arc<Self>> {
        McpAllowlist::from_env()
            .check(&endpoint)
            .map_err(|e| anyhow::anyhow!("remote MCP '{}': {e}", manifest.name))?;
        Ok(Arc::new(Self { manifest, endpoint }))
    }
}

#[async_trait]
impl CapabilityProvider for RemoteMcpCapability {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        _tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        // Strip the "namespace.tool" prefix — MCP servers only know the bare tool name.
        let bare_name = tool_name.rsplit_once('.').map_or(tool_name, |(_, n)| n);
        let adapter = McpAdapter::new(&self.endpoint)
            .map_err(|e| anyhow::anyhow!("MCP adapter init error: {e}"))?;
        adapter
            .call_tool(bare_name, input.clone())
            .await
            .map_err(|e| anyhow::anyhow!("MCP call_tool error: {e}"))
    }
}
