pub mod builtin;
pub mod chain;
pub mod mcp;
pub mod wasm;

use crate::tools::card::ToolCard;
use crate::tools::manifest::ToolKind;
use crate::tools::provider::ToolProvider;
use std::sync::Arc;

/// Instantiate the right provider for a given card based on its kind + name.
/// Returns `Err` only for Docker (reserved) or unknown chain names.
pub fn provider_for(card: ToolCard) -> anyhow::Result<Arc<dyn ToolProvider>> {
    Ok(match card.manifest.kind {
        ToolKind::Mcp => Arc::new(mcp::McpProvider::new(card)?),
        ToolKind::Wasm => Arc::new(wasm::WasmProvider::new(card)),
        ToolKind::Chain => match card.manifest.name.as_str() {
            "invoice-processing" => Arc::new(chain::InvoiceProvider::new(card)),
            "contract-processing" => Arc::new(chain::ContractProvider::new(card)),
            "ocr-service" => Arc::new(chain::OcrProvider::new(card)),
            other => anyhow::bail!("unknown chain tool: {other}"),
        },
        ToolKind::Docker => anyhow::bail!("Docker kind is reserved"),
        ToolKind::Native => Arc::new(builtin::BuiltinProvider::new()),
    })
}
