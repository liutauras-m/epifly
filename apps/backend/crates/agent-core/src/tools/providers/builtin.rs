use crate::context::tenant::TenantContext;
use crate::tools::builtin::{cargo, fs};
use crate::tools::card::ToolCard;
use crate::tools::manifest::{ToolKind, ToolManifest};
use crate::tools::provider::{ToolProvider, ToolProviderFactory};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

/// Provider for built-in native tools (filesystem + cargo).
pub struct BuiltinProvider {
    manifest: ToolManifest,
}

impl Default for BuiltinProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl BuiltinProvider {
    pub fn new() -> Self {
        let card = crate::tools::builtin::card::builtin_tool_card();
        Self {
            manifest: card.manifest,
        }
    }
}

#[async_trait]
impl ToolProvider for BuiltinProvider {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        let workspace_root = tenant
            .map(|t| t.workspace_root.to_string_lossy().to_string())
            .unwrap_or_else(|| {
                std::env::var("CONUSAI_WORKSPACE_ROOT")
                    .unwrap_or_else(|_| "/tmp/conusai/workspaces".into())
            });

        match tool_name {
            "read_file" => fs::read_file(&workspace_root, input).await,
            "write_file" => fs::write_file(&workspace_root, input).await,
            "run_cargo" => {
                let root = tenant
                    .map(|t| t.workspace_root.to_string_lossy().to_string())
                    .unwrap_or_else(|| {
                        std::env::var("CONUSAI_WORKSPACE_ROOT").unwrap_or_else(|_| ".".into())
                    });
                cargo::run_cargo(&root, input).await
            }
            other => anyhow::bail!("unknown builtin tool: {other}"),
        }
    }
}

/// Factory for `ToolKind::Native` — creates a `BuiltinProvider`.
pub struct BuiltinFactory;

impl ToolProviderFactory for BuiltinFactory {
    fn supports(&self, kind: &ToolKind, _name: &str) -> bool {
        matches!(kind, ToolKind::Native)
    }

    fn create(&self, _card: ToolCard) -> anyhow::Result<Arc<dyn ToolProvider>> {
        Ok(Arc::new(BuiltinProvider::new()))
    }
}
