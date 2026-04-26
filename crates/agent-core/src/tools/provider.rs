use crate::context::tenant::TenantContext;
use crate::tools::manifest::ToolManifest;
use async_trait::async_trait;
use serde_json::Value;

/// Anything that can execute one or more named tools given a JSON input.
///
/// Implementors hold their own state (HTTP clients, WASM engines, model handles).
/// The registry keeps `Arc<dyn ToolProvider>` so providers can be cheap to clone
/// and shared across concurrent agent turns.
#[async_trait]
pub trait ToolProvider: Send + Sync + 'static {
    /// The manifest is the contract: name, kind, tool list, embedding text, etc.
    fn manifest(&self) -> &ToolManifest;

    /// Execute one tool and return its JSON output.
    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value>;

    /// Anthropic-format tool definitions; default implementation derives from the manifest.
    fn tool_definitions(&self) -> Vec<Value> {
        crate::tools::executor::tool_definitions_from_manifest(self.manifest())
    }
}
