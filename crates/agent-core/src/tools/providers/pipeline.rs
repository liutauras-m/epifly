use crate::context::tenant::TenantContext;
use crate::pipelines::contract::ContractPipeline;
use crate::pipelines::invoice::InvoicePipeline;
use crate::tools::card::ToolCard;
use crate::tools::executor::resolve_image_path;
use crate::tools::manifest::ToolManifest;
use crate::tools::provider::ToolProvider;
use async_trait::async_trait;
use serde_json::{Value, json};

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
                let mut pipeline = InvoicePipeline::with_model(model);
                if let Some(t) = tenant {
                    pipeline = pipeline.with_tenant(t.clone());
                }
                let result = pipeline.extract_from_image_path(&effective_path).await;
                if let Some(ref tmp) = temp_path {
                    let _ = std::fs::remove_file(tmp);
                }
                Ok(serde_json::to_value(result?)?)
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
                let mut pipeline = ContractPipeline::with_model(model);
                if let Some(t) = tenant {
                    pipeline = pipeline.with_tenant(t.clone());
                }
                let result = pipeline.extract_from_document_path(&effective_path).await;
                if let Some(ref tmp) = temp_path {
                    let _ = std::fs::remove_file(tmp);
                }
                Ok(serde_json::to_value(result?)?)
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
        _tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        match tool_name {
            "extract_text" => {
                let image_path = input["image_path"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("missing image_path"))?;
                let model = input["model"].as_str().unwrap_or("claude-opus-4-7");
                let (temp_path, effective_path) = resolve_image_path(image_path).await?;
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
            other => anyhow::bail!("unknown ocr tool: {other}"),
        }
    }
}
