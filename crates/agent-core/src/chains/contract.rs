use crate::context::tenant::TenantContext;
use base64::{Engine as _, engine::general_purpose};
use rig::OneOrMany;
use rig::completion::CompletionModel;
use rig::message::{ContentFormat, ImageMediaType, Message, UserContent};
use rig::providers::anthropic;
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractParty {
    pub name: String,
    pub role: Option<String>,
    pub address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractData {
    pub contract_type: Option<String>,
    pub parties: Vec<ContractParty>,
    pub effective_date: Option<String>,
    pub expiry_date: Option<String>,
    pub term_months: Option<f64>,
    pub governing_law: Option<String>,
    pub jurisdiction: Option<String>,
    pub key_obligations: Vec<String>,
    pub termination_clauses: Vec<String>,
    pub payment_terms: Option<String>,
    pub signatories: Vec<String>,
    pub summary: Option<String>,
}

pub struct ContractPipeline {
    model: anthropic::completion::CompletionModel,
    tenant: Option<TenantContext>,
}

impl ContractPipeline {
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

    fn resolve_path(&self, path: &std::path::Path) -> common::error::Result<std::path::PathBuf> {
        if let Some(tenant) = &self.tenant {
            if path.is_absolute() {
                return Ok(path.to_path_buf());
            }
            tenant.safe_path(&path.to_string_lossy())
        } else {
            Ok(path.to_path_buf())
        }
    }

    #[instrument(skip(self), fields(
        tenant_id = self.tenant.as_ref().map(|t| t.tenant_id.as_str()).unwrap_or("none"),
        path = %path.display()
    ))]
    pub async fn extract_from_document_path(
        &self,
        path: &std::path::Path,
    ) -> common::error::Result<ContractData> {
        let resolved = self.resolve_path(path)?;
        info!(resolved = %resolved.display(), "reading contract document");
        let bytes = std::fs::read(&resolved).map_err(|e| {
            common::error::ConusAiError::Tool(format!("cannot read document {:?}: {e}", resolved))
        })?;
        self.run_extraction(&bytes).await
    }

    /// Convenience: extract from a byte slice without tenant context.
    pub async fn extract_from_bytes(&self, bytes: &[u8]) -> common::error::Result<ContractData> {
        self.run_extraction(bytes).await
    }

    /// Core extraction — base64-encodes bytes and sends to Claude vision.
    async fn run_extraction(&self, bytes: &[u8]) -> common::error::Result<ContractData> {
        let b64 = general_purpose::STANDARD.encode(bytes);

        let content = OneOrMany::many(vec![
            UserContent::image(b64, Some(ContentFormat::Base64), Some(ImageMediaType::PNG), None),
            UserContent::text(
                "Extract all contract information from this legal document. \
                Return a valid JSON object matching exactly this structure — \
                use null for missing scalar fields, empty arrays [] for missing lists:\n\
                {\
                  \"contract_type\": string|null,\
                  \"parties\": [{\"name\": string, \"role\": string|null, \"address\": string|null}],\
                  \"effective_date\": string|null,\
                  \"expiry_date\": string|null,\
                  \"term_months\": number|null,\
                  \"governing_law\": string|null,\
                  \"jurisdiction\": string|null,\
                  \"key_obligations\": [string],\
                  \"termination_clauses\": [string],\
                  \"payment_terms\": string|null,\
                  \"signatories\": [string],\
                  \"summary\": string|null\
                }\n\
                Respond ONLY with the JSON object, no markdown fences, no explanation.",
            ),
        ])
        .map_err(|e| common::error::ConusAiError::Tool(e.to_string()))?;

        let msg = Message::User { content };

        let request = self
            .model
            .completion_request(msg)
            .preamble(
                "You are a legal document analysis specialist. \
                Extract structured data from contracts with high accuracy. \
                Always respond with valid JSON only."
                    .to_string(),
            )
            .max_tokens(2048)
            .build();

        let response = self.model.completion(request).await.map_err(|e| {
            error!(error = %e, "Claude completion failed for contract extraction");
            common::error::ConusAiError::Tool(e.to_string())
        })?;

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
        serde_json::from_str::<ContractData>(json_text).map_err(|e| {
            error!(error = %e, raw = %text, "failed to parse contract JSON response");
            common::error::ConusAiError::Tool(format!(
                "failed to parse contract JSON: {e}\nRaw response:\n{text}"
            ))
        })
    }
}

impl Default for ContractPipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl super::extraction::ExtractionPipeline for ContractPipeline {
    type Output = ContractData;

    fn model_id(&self) -> &str {
        "claude-opus-4-7"
    }

    fn system_prompt(&self) -> &str {
        "You are a legal document analysis specialist. \
        Extract structured data from contracts with high accuracy. \
        Always respond with valid JSON only."
    }

    async fn run(
        &self,
        bytes: Vec<u8>,
        _tenant: Option<&TenantContext>,
    ) -> common::error::Result<Self::Output> {
        self.run_extraction(&bytes).await
    }
}

fn strip_markdown_fences(s: &str) -> &str {
    let s = s.trim();
    let s = s.strip_prefix("```json").unwrap_or(s);
    let s = s.strip_prefix("```").unwrap_or(s);
    let s = s.strip_suffix("```").unwrap_or(s);
    s.trim()
}
