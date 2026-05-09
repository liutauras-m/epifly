use crate::llm::error::LlmError;
use crate::llm::types::{LlmRequest, LlmResponse, LlmStream};
use async_trait::async_trait;

/// A provider-agnostic LLM backend.
///
/// All routes, chains, and memory helpers must go through this trait —
/// **never** construct provider clients directly.
///
/// Implementation notes
/// - The trait is dyn-safe: all async methods use `#[async_trait]`.
/// - Default capability flags are conservative (`supports_vision = false`).
#[async_trait]
pub trait CompletionProvider: Send + Sync {
    fn name(&self) -> &'static str;

    fn supports_tools(&self) -> bool {
        true
    }
    fn supports_vision(&self) -> bool {
        false
    }
    fn supports_streaming(&self) -> bool {
        true
    }

    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError>;

    async fn stream(&self, req: LlmRequest) -> Result<LlmStream, LlmError>;
}
