use crate::context::tenant::TenantContext;
use async_trait::async_trait;
use serde_json::Value;

/// Common interface for document-extraction pipelines.
///
/// `InvoicePipeline` and `ContractPipeline` both follow the same pattern:
/// base64-encode bytes, send to Claude vision with a strict-JSON prompt,
/// deserialize into a typed struct.  This trait captures that shape so adding
/// a 4th pipeline type is a single-file change.
///
/// The concrete `extract_from_image_path` / `extract_from_document_path` helpers
/// on each pipeline remain as inherent methods — their signatures differ enough
/// (different path semantics, different error messages) that abstracting them
/// here would add noise without benefit.
#[async_trait]
pub trait ExtractionPipeline: Send + Sync {
    type Output: serde::de::DeserializeOwned + serde::Serialize + Send;

    fn model_id(&self) -> &str;

    /// The system prompt sent to Claude for this pipeline.
    fn system_prompt(&self) -> &str;

    /// Extract structured data from raw bytes (any supported MIME type).
    async fn extract_from_bytes(
        &self,
        bytes: &[u8],
        tenant: Option<&TenantContext>,
    ) -> common::error::Result<Self::Output>;

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
