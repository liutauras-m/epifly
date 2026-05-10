use crate::capabilities::card::CapabilityCard;
use crate::capabilities::manifest::{ToolKind, ToolManifest};
use crate::context::tenant::TenantContext;
use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::sync::Arc;

/// Anything that can execute one or more named tools given a JSON input.
///
/// Implementors hold their own state (HTTP clients, WASM engines, model handles).
/// The registry keeps `Arc<dyn CapabilityProvider>` so providers can be cheap to clone
/// and shared across concurrent agent turns.
#[async_trait]
pub trait CapabilityProvider: Send + Sync + 'static {
    /// The manifest is the contract: name, kind, tool list, embedding text, etc.
    fn manifest(&self) -> &ToolManifest;

    /// Execute one tool and return its JSON output.
    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value>;

    /// Typed convenience: serializes `input` to JSON, calls `invoke`, deserializes result.
    /// Default impl — no provider needs to override.
    ///
    /// `where Self: Sized` keeps the trait dyn-compatible; call the free function
    /// `invoke_typed_dyn` when you only have `&dyn CapabilityProvider`.
    async fn invoke_typed<I, O>(
        &self,
        tool_name: &str,
        input: I,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<O>
    where
        Self: Sized,
        I: Serialize + Send,
        O: DeserializeOwned + Send,
    {
        let v = self
            .invoke(tool_name, &serde_json::to_value(input)?, tenant)
            .await?;
        Ok(serde_json::from_value(v)?)
    }

    /// Anthropic-format tool definitions; default implementation derives from the manifest.
    fn tool_definitions(&self) -> Vec<Value> {
        crate::capabilities::executor::tool_definitions_from_manifest(self.manifest())
    }
}

/// Free-function equivalent of `invoke_typed` for use with `&dyn CapabilityProvider`.
///
/// Prefer calling `.invoke_typed()` directly on a concrete provider.  Use this
/// when you only have an `Arc<dyn CapabilityProvider>` or `&dyn CapabilityProvider`.
pub async fn invoke_typed_dyn<I, O>(
    provider: &dyn CapabilityProvider,
    tool_name: &str,
    input: I,
    tenant: Option<&TenantContext>,
) -> anyhow::Result<O>
where
    I: Serialize + Send,
    O: DeserializeOwned + Send,
{
    let v = provider
        .invoke(tool_name, &serde_json::to_value(input)?, tenant)
        .await?;
    Ok(serde_json::from_value(v)?)
}

/// Factory trait for creating providers from tool cards.
///
/// Implement this for each `ToolKind` so the registry can instantiate providers
/// dynamically.  Adding a new capability kind = one new file + one
/// `registry.register_factory(MyFactory)` call — the registry, executor, and
/// agent loop never need to change.
pub trait CapabilityFactory: Send + Sync + 'static {
    /// Returns true when this factory can handle the given kind + name combination.
    fn supports(&self, kind: &ToolKind, name: &str) -> bool;

    /// Create a provider from the card.  Called once per discovered capability.
    fn create(&self, card: CapabilityCard) -> anyhow::Result<Arc<dyn CapabilityProvider>>;
}

/// Factory variant that can load many capabilities efficiently from a data source
/// (e.g. a database table) in a single pass with batched embedding writes.
///
/// Implement this on `CapabilityFactory` implementors that have bulk sources.
#[async_trait]
pub trait BulkCapabilityFactory: CapabilityFactory {
    /// Load all available capabilities into `registry`.
    /// Returns the count of successfully loaded capabilities.
    async fn load_batch(
        &self,
        into: &mut crate::capabilities::registry::CapabilityRegistry,
    ) -> anyhow::Result<usize>;
}
