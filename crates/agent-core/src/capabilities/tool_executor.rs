use super::card::CapabilityCard;
use super::mcp_adapter::McpAdapter;
use super::wasm_loader::WasmCapabilityLoader;
use crate::capabilities::manifest::CapabilityKind;
use crate::context::tenant::TenantContext;
use crate::pipelines::contract::ContractPipeline;
use crate::pipelines::invoice::InvoicePipeline;
use crate::tools::{cargo_tool, fs_tools};
use common::metrics;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Instant;
use tracing::{Span, instrument};

pub struct CapabilityExecutor;

impl CapabilityExecutor {
    /// Dispatch a tool call to the appropriate backend.
    ///
    /// Handles: invoice-processing, ocr-service (pipeline), wasm-ping (wasmtime).
    /// File-storage is handled in the gateway (requires MinIO client).
    #[instrument(
        skip(card, input, tenant),
        fields(
            capability.name  = %card.manifest.name,
            tool.name        = tool_name,
            tool.kind        = ?card.manifest.kind,
            error.type       = tracing::field::Empty,
        )
    )]
    pub async fn invoke(
        card: &CapabilityCard,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        let t0 = Instant::now();
        let cap = card.manifest.name.as_str();
        let labels = [
            metrics::kv("capability", cap),
            metrics::kv("tool", tool_name),
        ];
        metrics::tool_invocations().add(1, &labels);

        let result = Self::dispatch(card, tool_name, input, tenant).await;

        let elapsed = t0.elapsed().as_secs_f64() * 1000.0;
        metrics::tool_duration_ms().record(elapsed, &labels);

        if let Err(ref e) = result {
            metrics::tool_errors().add(1, &labels);
            metrics::record_error(&Span::current(), e);
        }

        result
    }

    /// Internal dispatch — called from `invoke` after metrics setup.
    async fn dispatch(
        card: &CapabilityCard,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        match (card.manifest.name.as_str(), tool_name) {
            // ── Contract pipeline ────────────────────────────────────────────
            ("contract-processing", "extract_contract") => {
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

            ("contract-processing", "summarise_contract") => {
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
            // ── Invoice pipeline ─────────────────────────────────────────────
            ("invoice-processing", "extract_invoice") => {
                let image_path = input["image_path"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("missing image_path"))?;
                let model = input["model"].as_str().unwrap_or("claude-opus-4-7");

                // If a URL is provided, download to a temp file first
                let (temp_path, effective_path) = resolve_image_path(image_path).await?;

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

            // ── Native (built-in) tools ──────────────────────────────────────
            ("native-tools", "read_file") => {
                let workspace_root = tenant
                    .map(|t| t.workspace_root.to_string_lossy().to_string())
                    .unwrap_or_else(|| {
                        std::env::var("CONUSAI_WORKSPACE_ROOT")
                            .unwrap_or_else(|_| "/tmp/conusai/workspaces".into())
                    });
                fs_tools::read_file(&workspace_root, input).await
            }

            ("native-tools", "write_file") => {
                let workspace_root = tenant
                    .map(|t| t.workspace_root.to_string_lossy().to_string())
                    .unwrap_or_else(|| {
                        std::env::var("CONUSAI_WORKSPACE_ROOT")
                            .unwrap_or_else(|_| "/tmp/conusai/workspaces".into())
                    });
                fs_tools::write_file(&workspace_root, input).await
            }

            ("native-tools", "run_cargo") => {
                let workspace_root = tenant
                    .map(|t| t.workspace_root.to_string_lossy().to_string())
                    .unwrap_or_else(|| {
                        std::env::var("CONUSAI_WORKSPACE_ROOT").unwrap_or_else(|_| ".".into())
                    });
                cargo_tool::run_cargo(&workspace_root, input).await
            }

            // ── External MCP server federation ───────────────────────────────
            // Any `kind: mcp` capability with `config.endpoint` set is forwarded
            // to the remote MCP server via JSON-RPC 2.0.
            (_cap_name, tool) if card.manifest.kind == CapabilityKind::Mcp => {
                let endpoint = card.manifest.config["endpoint"].as_str().ok_or_else(|| {
                    anyhow::anyhow!(
                        "MCP capability '{}' has no config.endpoint — \
                        add `endpoint: http://...` to its capability.yaml config section",
                        card.manifest.name
                    )
                })?;
                let adapter = McpAdapter::new(endpoint).map_err(|e| anyhow::anyhow!("{e}"))?;
                adapter
                    .call_tool(tool, input.clone())
                    .await
                    .map_err(|e| anyhow::anyhow!("{e}"))
            }

            // ── WASM capabilities ────────────────────────────────────────────
            (_cap_name, tool) if card.manifest.kind == CapabilityKind::Wasm => {
                let loader = WasmCapabilityLoader::new().map_err(|e| anyhow::anyhow!("{e}"))?;
                loader
                    .invoke_tool(card, tool, input)
                    .map_err(|e| anyhow::anyhow!("{e}"))
            }

            // ── Docker capabilities (reserved) ───────────────────────────────
            (_cap, _tool) if card.manifest.kind == CapabilityKind::Docker => {
                anyhow::bail!(
                    "Docker capability kind is reserved and not yet executable. \
                    Implement a container runner in tool_executor.rs to enable it."
                )
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
async fn resolve_image_path(image_path: &str) -> anyhow::Result<(Option<PathBuf>, PathBuf)> {
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

        let tmp = std::env::temp_dir().join(format!("conusai-{}.{ext}", uuid::Uuid::new_v4()));
        std::fs::write(&tmp, &bytes).map_err(|e| anyhow::anyhow!("write temp file failed: {e}"))?;

        Ok((Some(tmp.clone()), tmp))
    } else {
        Ok((None, Path::new(image_path).to_path_buf()))
    }
}
