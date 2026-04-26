use crate::context::tenant::TenantContext;
use crate::tools::card::ToolCard;
use crate::tools::manifest::{ToolKind, ToolManifest};
use crate::tools::provider::{ToolProvider, ToolProviderFactory};
use crate::tools::wasm_loader::WasmToolLoader;
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

pub struct WasmProvider {
    card: ToolCard,
    manifest: ToolManifest,
}

impl WasmProvider {
    pub fn new(card: ToolCard) -> Self {
        let manifest = card.manifest.clone();
        Self { card, manifest }
    }
}

#[async_trait]
impl ToolProvider for WasmProvider {
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
            .map_err(|e| anyhow::anyhow!("{e}"))
    }
}

/// Factory for `ToolKind::Wasm`.
pub struct WasmFactory;

impl ToolProviderFactory for WasmFactory {
    fn supports(&self, kind: &ToolKind, _name: &str) -> bool {
        matches!(kind, ToolKind::Wasm)
    }

    fn create(&self, card: ToolCard) -> anyhow::Result<Arc<dyn ToolProvider>> {
        Ok(Arc::new(WasmProvider::new(card)))
    }
}
