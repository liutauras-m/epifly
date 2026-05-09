//! `TranscribeVideoCapability` — a `CapabilityProvider` that enqueues a
//! `VideoTranscriptionJob` via the `JobExecutor` and returns a `task_id` instantly.
//!
//! Register it by adding it to the `ToolRegistry` at startup with a synthetic
//! `CapabilityCard` built by `transcribe_video_card()`.

use agent_core::context::tenant::TenantContext;
use agent_core::tools::manifest::{ToolDef, ToolKind, ToolManifest};
use agent_core::tools::provider::CapabilityProvider;
use async_trait::async_trait;
use jobs::JobExecutor;
use serde_json::{Value, json};
use std::sync::Arc;

/// A `CapabilityProvider` that submits video-transcription jobs asynchronously.
pub struct TranscribeVideoCapability {
    manifest: ToolManifest,
    executor: Arc<JobExecutor>,
}

impl TranscribeVideoCapability {
    pub fn new(executor: Arc<JobExecutor>) -> Self {
        let input_schema = json!({
            "type": "object",
            "properties": {
                "file_id": {
                    "type": "string",
                    "description": "Object key returned by POST /v1/files (the download token)"
                }
            },
            "required": ["file_id"]
        });

        let manifest = ToolManifest {
            name: "transcribe-video".into(),
            version: "0.1.0".into(),
            description: "Transcribes an audio or video file stored in MinIO via the Whisper API. Returns a task_id immediately; poll GET /v1/tasks/{id} for the result.".into(),
            kind: ToolKind::Native,
            tools: vec![ToolDef {
                name: "transcribe".into(),
                description: "Enqueue a video/audio transcription job. Returns task_id and queued status.".into(),
                input_schema,
            }],
            config: serde_json::Value::Null,
            tags: vec!["audio".into(), "video".into(), "transcription".into()],
            chain: None,
        };

        Self { manifest, executor }
    }
}

#[async_trait]
impl CapabilityProvider for TranscribeVideoCapability {
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
            "transcribe" => {
                let file_id = input["file_id"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("missing required field: file_id"))?;

                let tenant_id = tenant
                    .map(|t| t.tenant_id.as_str().to_owned())
                    .unwrap_or_else(|| "__dev__".to_owned());

                let payload = json!({
                    "file_id": file_id,
                    "tenant_id": tenant_id,
                });

                let task_id = self.executor.enqueue("video-transcription", payload).await?;

                Ok(json!({
                    "task_id": task_id,
                    "status": "queued",
                    "poll_url": format!("/v1/tasks/{}", task_id),
                }))
            }
            _ => anyhow::bail!("unknown tool: {tool_name}"),
        }
    }
}
