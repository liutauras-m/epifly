//! `VideoTranscriptionJob` — transcribes an audio/video file from the tenant's
//! object storage via the OpenAI Whisper API (`OPENAI_API_KEY`). The file is read
//! with the tenant's own storage credentials; the OpenAI key is sent to OpenAI only.
//!
//! Input JSON: `{ "file_id": "<s3-object-key>", "tenant_id": "<string>" }`
//! Output JSON: `{ "text": "<transcript>", "file_id": "<output-key>" }`

use crate::context::JobContext;
use crate::job::BackgroundJob;
use agent_core::capabilities::object_source::TenantObjectSource;
use agent_core::{PlanTier, TenantContext};
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{info, instrument};

pub struct VideoTranscriptionJob;

#[async_trait]
impl BackgroundJob for VideoTranscriptionJob {
    fn name(&self) -> &str {
        "video-transcription"
    }

    #[instrument(skip(self, ctx), fields(job = self.name()))]
    async fn run(
        &self,
        input: serde_json::Value,
        ctx: Arc<JobContext>,
    ) -> anyhow::Result<serde_json::Value> {
        let file_id = input["file_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing field: file_id"))?
            .to_owned();

        let tenant_id = input["tenant_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing field: tenant_id"))?
            .to_owned();

        info!(file_id = %file_id, tenant_id = %tenant_id, "video-transcription: starting");

        let api_key = std::env::var("OPENAI_API_KEY")
            .ok()
            .filter(|k| !k.trim().is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!("transcription is not configured (OPENAI_API_KEY unset)")
            })?;
        let factory = ctx
            .tenant_storage_factory
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("object storage is not configured"))?;
        let tenant = TenantContext::new(tenant_id.as_str(), None::<String>, PlanTier::Free);
        let file_bytes = factory.read(&tenant, &file_id).await?;
        let transcript =
            transcribe_via_openai_whisper(&api_key, &file_id, file_bytes.to_vec()).await?;

        info!(
            file_id = %file_id,
            chars = transcript.len(),
            "video-transcription: completed"
        );

        Ok(serde_json::json!({
            "file_id": file_id,
            "tenant_id": tenant_id,
            "transcript": transcript,
            "chars": transcript.len(),
        }))
    }
}

async fn transcribe_via_openai_whisper(
    api_key: &str,
    file_id: &str,
    file_bytes: Vec<u8>,
) -> anyhow::Result<String> {
    // Call OpenAI Whisper transcription endpoint
    let form = reqwest::multipart::Form::new()
        .part(
            "file",
            reqwest::multipart::Part::bytes(file_bytes)
                .file_name(file_id.to_owned())
                .mime_str("audio/mpeg")?,
        )
        .text("model", "whisper-1");

    let client = reqwest::Client::new();
    let resp = client
        .post("https://api.openai.com/v1/audio/transcriptions")
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Whisper API error {status}: {body}");
    }

    let body: serde_json::Value = resp.json().await?;
    let text = body["text"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing 'text' in Whisper response"))?
        .to_owned();

    Ok(text)
}
