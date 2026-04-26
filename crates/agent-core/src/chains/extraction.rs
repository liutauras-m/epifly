use crate::context::tenant::TenantContext;
use async_trait::async_trait;
use serde_json::Value;

/// Common interface for document-extraction chains.
///
/// `InvoicePipeline` and `ContractPipeline` both follow the same pattern:
/// base64-encode bytes, send to Claude vision with a strict-JSON prompt,
/// deserialize into a typed struct.  This trait captures that shape so adding
/// a new chain type is a single-file change.
///
/// The primary method is `run()` — its signature mirrors `rig::pipeline::Op::call`
/// so a future multi-step `rig::pipeline::Chain<Output>` can plug in here with zero
/// changes to the trait or its implementors.
///
/// `extract_from_bytes` and `extract_as_value` are provided as default impls that
/// delegate to `run()`.  The concrete path-based helpers (`extract_from_image_path`,
/// `extract_from_document_path`) remain as inherent methods on each struct because
/// their signatures differ enough that abstracting them here would add noise without
/// benefit.
///
/// # Why not a real `rig::pipeline::Chain` today?
/// Both extraction chains are single-shot Claude vision calls.  There is no
/// multi-step composition to perform, and `rig::pipeline` 0.9 does not natively
/// handle multimodal (image bytes) inputs.  When a second step is added (e.g. OCR
/// fallback, validation pass) this trait's `run()` becomes the natural attachment
/// point for a real `Chain` — no changes to callers required.
#[async_trait]
pub trait ExtractionPipeline: Send + Sync {
    type Output: serde::de::DeserializeOwned + serde::Serialize + Send;

    fn model_id(&self) -> &str;

    /// The system prompt sent to Claude for this chain.
    fn system_prompt(&self) -> &str;

    /// Primary extraction entry point — mirrors `rig::pipeline::Op::call`.
    async fn run(
        &self,
        bytes: Vec<u8>,
        tenant: Option<&TenantContext>,
    ) -> common::error::Result<Self::Output>;

    /// Convenience: extract from a byte slice (delegates to `run`).
    async fn extract_from_bytes(
        &self,
        bytes: &[u8],
        tenant: Option<&TenantContext>,
    ) -> common::error::Result<Self::Output> {
        self.run(bytes.to_vec(), tenant).await
    }

    /// Convenience: build a `Value` from the output for use in tool results.
    async fn extract_as_value(
        &self,
        bytes: &[u8],
        tenant: Option<&TenantContext>,
    ) -> common::error::Result<Value>
    where
        Self::Output: serde::Serialize,
    {
        let output = self.extract_from_bytes(bytes, tenant).await?;
        serde_json::to_value(output).map_err(|e| common::error::ConusAiError::Tool(e.to_string()))
    }
}
