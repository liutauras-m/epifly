use crate::capabilities::card::CapabilityCard;
use crate::capabilities::manifest::{ToolKind, ToolManifest};
use crate::capabilities::provider::{CapabilityFactory, CapabilityProvider};
use crate::capabilities::wasm_loader::WasmToolLoader;
use crate::context::tenant::TenantContext;
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

pub struct WasmProvider {
    card: CapabilityCard,
    manifest: ToolManifest,
}

impl WasmProvider {
    pub fn new(card: CapabilityCard) -> Self {
        let manifest = card.manifest.clone();
        Self { card, manifest }
    }
}

#[async_trait]
impl CapabilityProvider for WasmProvider {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        _tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        let loader = WasmToolLoader::new().map_err(|e| anyhow::anyhow!("{e}"))?;
        loader
            .invoke_tool(&self.card, tool_name, input)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))
    }
}

/// Factory for `ToolKind::Wasm`.
pub struct WasmFactory;

impl CapabilityFactory for WasmFactory {
    fn supports(&self, kind: &ToolKind, _name: &str) -> bool {
        matches!(kind, ToolKind::Wasm)
    }

    fn create(&self, card: CapabilityCard) -> anyhow::Result<Arc<dyn CapabilityProvider>> {
        Ok(Arc::new(WasmProvider::new(card)))
    }
}
