use crate::context::tenant::TenantContext;
use base64::{engine::general_purpose, Engine as _};
use rig::completion::CompletionModel;
use rig::message::{ContentFormat, ImageMediaType, Message, UserContent};
use rig::providers::anthropic;
use rig::OneOrMany;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct InvoiceLineItem {
    pub description: String,
    pub quantity: Option<f64>,
    pub unit_price: Option<f64>,
    pub total: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct InvoiceData {
    pub invoice_number: Option<String>,
    pub invoice_date: Option<String>,
    pub due_date: Option<String>,
    pub issuer_name: Option<String>,
    pub issuer_address: Option<String>,
    pub issuer_vat: Option<String>,
    pub billed_to_name: Option<String>,
    pub billed_to_company: Option<String>,
    pub billed_to_address: Option<String>,
    pub billed_to_email: Option<String>,
    pub currency: Option<String>,
    pub subtotal: Option<f64>,
    pub tax_amount: Option<f64>,
    pub total_amount: Option<f64>,
    pub amount_due: Option<f64>,
    pub status: Option<String>,
    pub line_items: Vec<InvoiceLineItem>,
    pub notes: Option<String>,
    pub order_number: Option<String>,
}

pub struct InvoicePipeline {
    model: rig::providers::anthropic::completion::CompletionModel,
    tenant: Option<TenantContext>,
}

impl InvoicePipeline {
    pub fn new() -> Self {
        let client = anthropic::Client::from_env();
        Self {
            model: client.completion_model("claude-opus-4-7"),
            tenant: None,
        }
    }

    pub fn with_model(model_id: &str) -> Self {
        let client = anthropic::Client::from_env();
        Self {
            model: client.completion_model(model_id),
            tenant: None,
        }
    }

    pub fn with_tenant(mut self, tenant: TenantContext) -> Self {
        self.tenant = Some(tenant);
        self
    }

    /// For tenant runs, resolves `rel_path` under the tenant workspace.
    /// Falls back to treating it as a plain path for non-tenant usage.
    fn resolve_path(&self, path: &std::path::Path) -> common::error::Result<std::path::PathBuf> {
        if let Some(tenant) = &self.tenant {
            // If path is already absolute, use it directly (dev/test convenience)
            if path.is_absolute() {
                return Ok(path.to_path_buf());
            }
            let rel = path.to_string_lossy();
            tenant.safe_path(&rel)
        } else {
            Ok(path.to_path_buf())
        }
    }

    #[instrument(skip(self), fields(
        tenant_id = self.tenant.as_ref().map(|t| t.tenant_id.as_str()).unwrap_or("none"),
        path = %path.display()
    ))]
    pub async fn extract_from_image_path(
        &self,
        path: &std::path::Path,
    ) -> common::error::Result<InvoiceData> {
        let resolved = self.resolve_path(path)?;
        info!(resolved = %resolved.display(), "reading invoice image");
        let bytes = std::fs::read(&resolved).map_err(|e| {
            common::error::ConusAiError::Capability(format!(
                "cannot read image {:?}: {e}",
                resolved
            ))
        })?;
        self.extract_from_bytes(&bytes).await
    }

    pub async fn extract_from_bytes(&self, bytes: &[u8]) -> common::error::Result<InvoiceData> {
        let b64 = general_purpose::STANDARD.encode(bytes);

        let content = OneOrMany::many(vec![
            UserContent::image(b64, Some(ContentFormat::Base64), Some(ImageMediaType::PNG), None),
            UserContent::text(
                "Extract all invoice information from this image. \
                Return a valid JSON object matching exactly this structure — \
                use null for missing fields, empty array [] if no line items:\n\
                {\
                  \"invoice_number\": string|null,\
                  \"invoice_date\": string|null,\
                  \"due_date\": string|null,\
                  \"issuer_name\": string|null,\
                  \"issuer_address\": string|null,\
                  \"issuer_vat\": string|null,\
                  \"billed_to_name\": string|null,\
                  \"billed_to_company\": string|null,\
                  \"billed_to_address\": string|null,\
                  \"billed_to_email\": string|null,\
                  \"currency\": string|null,\
                  \"subtotal\": number|null,\
                  \"tax_amount\": number|null,\
                  \"total_amount\": number|null,\
                  \"amount_due\": number|null,\
                  \"status\": string|null,\
                  \"line_items\": [{\"description\": string, \"quantity\": number|null, \"unit_price\": number|null, \"total\": number|null}],\
                  \"notes\": string|null,\
                  \"order_number\": string|null\
                }\n\
                Respond ONLY with the JSON object, no markdown fences, no explanation.",
            ),
        ]).map_err(|e| common::error::ConusAiError::Capability(e.to_string()))?;

        let msg = Message::User { content };

        let request = self
            .model
            .completion_request(msg)
            .preamble(
                "You are an invoice data extraction specialist. \
                Extract structured data from invoice images with high accuracy. \
                Always respond with valid JSON only."
                    .to_string(),
            )
            .max_tokens(2048)
            .build();

        let response = self
            .model
            .completion(request)
            .await
            .map_err(|e| common::error::ConusAiError::Capability(e.to_string()))?;

        let text = response
            .choice
            .iter()
            .filter_map(|c| {
                if let rig::completion::message::AssistantContent::Text(t) = c {
                    Some(t.text.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("");

        let json_text = strip_markdown_fences(&text);
        serde_json::from_str::<InvoiceData>(json_text).map_err(|e| {
            common::error::ConusAiError::Capability(format!(
                "failed to parse invoice JSON: {e}\nRaw response:\n{text}"
            ))
        })
    }
}

fn strip_markdown_fences(s: &str) -> &str {
    let s = s.trim();
    let s = s.strip_prefix("```json").unwrap_or(s);
    let s = s.strip_prefix("```").unwrap_or(s);
    let s = s.strip_suffix("```").unwrap_or(s);
    s.trim()
}

impl Default for InvoicePipeline {
    fn default() -> Self {
        Self::new()
    }
}
