pub mod builtin;
pub mod mcp;
pub mod pipeline;
pub mod wasm;

use crate::tools::card::ToolCard;
use crate::tools::manifest::ToolKind;
use crate::tools::provider::ToolProvider;
use std::sync::Arc;

/// Instantiate the right provider for a given card based on its kind + name.
/// Returns `Err` only for Docker (reserved) or unknown pipeline names.
pub fn provider_for(card: ToolCard) -> anyhow::Result<Arc<dyn ToolProvider>> {
    Ok(match card.manifest.kind {
        ToolKind::Mcp => Arc::new(mcp::McpProvider::new(card)?),
        ToolKind::Wasm => Arc::new(wasm::WasmProvider::new(card)),
        ToolKind::Pipeline => match card.manifest.name.as_str() {
            "invoice-processing" => Arc::new(pipeline::InvoiceProvider::new(card)),
            "contract-processing" => Arc::new(pipeline::ContractProvider::new(card)),
            "ocr-service" => Arc::new(pipeline::OcrProvider::new(card)),
            other => anyhow::bail!("unknown pipeline tool: {other}"),
        },
        ToolKind::Docker => anyhow::bail!("Docker kind is reserved"),
        ToolKind::Native => Arc::new(builtin::BuiltinProvider::new()),
    })
}
