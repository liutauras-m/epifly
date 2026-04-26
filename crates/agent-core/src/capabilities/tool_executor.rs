use super::card::CapabilityCard;
use super::wasm_loader::WasmCapabilityLoader;
use crate::context::tenant::TenantContext;
use crate::pipelines::invoice::InvoicePipeline;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub struct CapabilityExecutor;

impl CapabilityExecutor {
    /// Dispatch a tool call to the appropriate backend.
    ///
    /// Handles: invoice-processing, ocr-service (pipeline), wasm-ping (wasmtime).
    /// File-storage is handled in the gateway (requires MinIO client).
    pub async fn invoke(
        card: &CapabilityCard,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        match (card.manifest.name.as_str(), tool_name) {
            // ── Invoice pipeline ─────────────────────────────────────────────
            ("invoice-processing", "extract_invoice") => {
                let image_path = input["image_path"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("missing image_path"))?;
                let model = input["model"].as_str().unwrap_or("claude-opus-4-7");

                // If a URL is provided, download to a temp file first
                let (temp_path, effective_path) =
                    resolve_image_path(image_path).await?;

                let mut pipeline = InvoicePipeline::with_model(model);
                if let Some(t) = tenant {
                    pipeline = pipeline.with_tenant(t.clone());
                }

                let result = pipeline.extract_from_image_path(&effective_path).await;

                // Always clean up temp file
                if let Some(ref tmp) = temp_path {
                    let _ = std::fs::remove_file(tmp);
                }

                Ok(serde_json::to_value(result?)?)
            }

            ("invoice-processing", "validate_invoice") => {
                let invoice = &input["invoice_data"];
                let has_number = invoice["invoice_number"].as_str().is_some();
                let has_total = invoice["total_amount"].as_f64().is_some();
                Ok(json!({
                    "valid": has_number && has_total,
                    "issues": if !has_number { vec!["missing invoice_number"] } else { vec![] }
                }))
            }

            // ── OCR service ──────────────────────────────────────────────────
            ("ocr-service", "extract_text") => {
                let image_path = input["image_path"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("missing image_path"))?;
                let model = input["model"].as_str().unwrap_or("claude-opus-4-7");

                let (temp_path, effective_path) =
                    resolve_image_path(image_path).await?;

                let pipeline = InvoicePipeline::with_model(model);
                let result = pipeline.extract_from_image_path(&effective_path).await;

                if let Some(ref tmp) = temp_path {
                    let _ = std::fs::remove_file(tmp);
                }

                let data = result?;
                Ok(json!({
                    "text": format!(
                        "Invoice #{} | {} → {} | {} {:.2} | Status: {}",
                        data.invoice_number.as_deref().unwrap_or("?"),
                        data.issuer_name.as_deref().unwrap_or("?"),
                        data.billed_to_name.as_deref().unwrap_or("?"),
                        data.currency.as_deref().unwrap_or(""),
                        data.total_amount.unwrap_or(0.0),
                        data.status.as_deref().unwrap_or("unknown"),
                    )
                }))
            }

            // ── WASM capabilities ────────────────────────────────────────────
            (_cap_name, tool)
                if card.manifest.kind == crate::capabilities::manifest::CapabilityKind::Wasm =>
            {
                let loader = WasmCapabilityLoader::new().map_err(|e| anyhow::anyhow!("{e}"))?;
                loader
                    .invoke_tool(card, tool, input)
                    .map_err(|e| anyhow::anyhow!("{e}"))
            }

            // ── Unknown ──────────────────────────────────────────────────────
            (cap, tool) => {
                anyhow::bail!("No executor registered for capability '{cap}' tool '{tool}'")
            }
        }
    }

    /// Build Anthropic-format tool definitions from a capability card.
    pub fn tool_definitions(card: &CapabilityCard) -> Vec<Value> {
        card.manifest
            .tools
            .iter()
            .map(|t| {
                json!({
                    "name": format!("{}__{}", card.manifest.name, t.name),
                    "description": t.description,
                    "input_schema": t.input_schema
                })
            })
            .collect()
    }
}

/// If `image_path` is a URL, download it to a temp file and return `(Some(temp), temp_path)`.
/// If it's a local path, return `(None, original_path)`.
async fn resolve_image_path(
    image_path: &str,
) -> anyhow::Result<(Option<PathBuf>, PathBuf)> {
    if image_path.starts_with("http://") || image_path.starts_with("https://") {
        let bytes = reqwest::get(image_path)
            .await
            .map_err(|e| anyhow::anyhow!("download failed: {e}"))?
            .bytes()
            .await
            .map_err(|e| anyhow::anyhow!("read body failed: {e}"))?;

        // Guess extension from URL
        let ext = if image_path.contains(".pdf") {
            "pdf"
        } else if image_path.contains(".jpg") || image_path.contains(".jpeg") {
            "jpg"
        } else {
            "png"
        };

        let tmp = std::env::temp_dir().join(format!(
            "conusai-{}.{ext}",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&tmp, &bytes)
            .map_err(|e| anyhow::anyhow!("write temp file failed: {e}"))?;

        Ok((Some(tmp.clone()), tmp))
    } else {
        Ok((None, Path::new(image_path).to_path_buf()))
    }
}
