//! Chain-kind tool providers — thin adapters that bridge `ToolProvider::invoke`
//! to the concrete extraction pipelines (`InvoicePipeline`, `ContractPipeline`).

use crate::chains::contract::ContractPipeline;
use crate::chains::invoice::InvoicePipeline;
use crate::context::tenant::TenantContext;
use crate::tools::card::ToolCard;
use crate::tools::executor::resolve_image_path;
use crate::tools::manifest::{ToolKind, ToolManifest};
use crate::tools::provider::{ToolProvider, ToolProviderFactory};
use async_trait::async_trait;
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::error;

// ── Invoice ──────────────────────────────────────────────────────────────────

pub struct InvoiceProvider {
    manifest: ToolManifest,
}

impl InvoiceProvider {
    pub fn new(card: ToolCard) -> Self {
        Self {
            manifest: card.manifest,
        }
    }
}

#[async_trait]
impl ToolProvider for InvoiceProvider {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        match tool_name {
            "extract_invoice" => {
                let image_path = input["image_path"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("missing image_path"))?;
                let model = input["model"].as_str().unwrap_or("claude-opus-4-7");
                let (temp_path, effective_path) = resolve_image_path(image_path).await?;
                let mut chain = InvoicePipeline::with_model(model);
                if let Some(t) = tenant {
                    chain = chain.with_tenant(t.clone());
                }
                let result = chain.extract_from_image_path(&effective_path).await;
                if let Some(ref tmp) = temp_path {
                    let _ = std::fs::remove_file(tmp);
                }
                result
                    .map(|d| serde_json::to_value(d).unwrap_or_default())
                    .map_err(|e| {
                        error!(
                            tenant_id = tenant.map(|t| t.tenant_id.as_str()).unwrap_or("none"),
                            tool = tool_name,
                            error = %e,
                            "invoice chain invocation failed"
                        );
                        anyhow::anyhow!("invoice extraction failed: {e}")
                    })
            }
            "validate_invoice" => {
                let invoice = &input["invoice_data"];
                let has_number = invoice["invoice_number"].as_str().is_some();
                let has_total = invoice["total_amount"].as_f64().is_some();
                Ok(json!({
                    "valid": has_number && has_total,
                    "issues": if !has_number { vec!["missing invoice_number"] } else { vec![] }
                }))
            }
            other => anyhow::bail!("unknown invoice tool: {other}"),
        }
    }
}

// ── Contract ─────────────────────────────────────────────────────────────────

pub struct ContractProvider {
    manifest: ToolManifest,
}

impl ContractProvider {
    pub fn new(card: ToolCard) -> Self {
        Self {
            manifest: card.manifest,
        }
    }
}

#[async_trait]
impl ToolProvider for ContractProvider {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        match tool_name {
            "extract_contract" => {
                let doc_path = input["document_path"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("missing document_path"))?;
                let model = input["model"].as_str().unwrap_or("claude-opus-4-7");
                let (temp_path, effective_path) = resolve_image_path(doc_path).await?;
                let mut chain = ContractPipeline::with_model(model);
                if let Some(t) = tenant {
                    chain = chain.with_tenant(t.clone());
                }
                let result = chain.extract_from_document_path(&effective_path).await;
                if let Some(ref tmp) = temp_path {
                    let _ = std::fs::remove_file(tmp);
                }
                result
                    .map(|d| serde_json::to_value(d).unwrap_or_default())
                    .map_err(|e| {
                        error!(
                            tenant_id = tenant.map(|t| t.tenant_id.as_str()).unwrap_or("none"),
                            tool = tool_name,
                            error = %e,
                            "contract chain invocation failed"
                        );
                        anyhow::anyhow!("contract extraction failed: {e}")
                    })
            }
            "summarise_contract" => {
                let contract = &input["contract_data"];
                let parties = contract["parties"]
                    .as_array()
                    .map(|ps| {
                        ps.iter()
                            .filter_map(|p| p["name"].as_str().map(|n| n.to_string()))
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                let contract_type = contract["contract_type"]
                    .as_str()
                    .unwrap_or("Unknown")
                    .to_string();
                let obligations = contract["key_obligations"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| format!("- {s}")))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let termination_clauses: Vec<String> = contract["termination_clauses"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                let summary = format!(
                    "{contract_type} between {parties}. Effective: {}. Expires: {}. Governed by: {}.",
                    contract["effective_date"]
                        .as_str()
                        .unwrap_or("not specified"),
                    contract["expiry_date"].as_str().unwrap_or("not specified"),
                    contract["governing_law"]
                        .as_str()
                        .unwrap_or("not specified"),
                );
                Ok(json!({
                    "summary": summary,
                    "risk_flags": termination_clauses,
                    "action_items": obligations,
                }))
            }
            other => anyhow::bail!("unknown contract tool: {other}"),
        }
    }
}

// ── OCR (reuses InvoicePipeline for vision) ──────────────────────────────────

pub struct OcrProvider {
    manifest: ToolManifest,
}

impl OcrProvider {
    pub fn new(card: ToolCard) -> Self {
        Self {
            manifest: card.manifest,
        }
    }
}

#[async_trait]
impl ToolProvider for OcrProvider {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        match tool_name {
            "extract_text" => {
                let image_path = input["image_path"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("missing image_path"))?;
                let model = input["model"].as_str().unwrap_or("claude-opus-4-7");
                let (temp_path, effective_path) = resolve_image_path(image_path).await?;
                let chain = InvoicePipeline::with_model(model);
                let result = chain.extract_from_image_path(&effective_path).await;
                if let Some(ref tmp) = temp_path {
                    let _ = std::fs::remove_file(tmp);
                }
                result
                    .map(|data| {
                        json!({
                            "text": format!(
                                "Invoice #{} | {} → {} | {} {:.2} | Status: {}",
                                data.invoice_number.as_deref().unwrap_or("?"),
                                data.issuer_name.as_deref().unwrap_or("?"),
                                data.billed_to_name.as_deref().unwrap_or("?"),
                                data.currency.as_deref().unwrap_or(""),
                                data.total_amount.unwrap_or(0.0),
                                data.status.as_deref().unwrap_or("unknown"),
                            )
                        })
                    })
                    .map_err(|e| {
                        error!(
                            tenant_id = tenant.map(|t| t.tenant_id.as_str()).unwrap_or("none"),
                            tool = tool_name,
                            error = %e,
                            "ocr chain invocation failed"
                        );
                        anyhow::anyhow!("ocr extraction failed: {e}")
                    })
            }
            other => anyhow::bail!("unknown ocr tool: {other}"),
        }
    }
}

/// Factory for `ToolKind::Chain` — routes by manifest name to the right provider.
pub struct ChainFactory;

impl ToolProviderFactory for ChainFactory {
    fn supports(&self, kind: &ToolKind, _name: &str) -> bool {
        matches!(kind, ToolKind::Chain)
    }

    fn create(&self, card: ToolCard) -> anyhow::Result<Arc<dyn ToolProvider>> {
        match card.manifest.name.as_str() {
            "invoice-processing" => Ok(Arc::new(InvoiceProvider::new(card))),
            "contract-processing" => Ok(Arc::new(ContractProvider::new(card))),
            "ocr-service" => Ok(Arc::new(OcrProvider::new(card))),
            other => anyhow::bail!("unknown chain tool: {other}"),
        }
    }
}
