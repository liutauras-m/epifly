//! Gateway-side helpers for constructing job-backed capability providers.
//!
//! The generic `JobBackedProvider` and `JobDispatch` trait live in
//! `agent-core/src/capabilities/providers/job_backed.rs` per §0 canonical placement.
//! This module provides the gateway-level convenience constructor that wires
//! `Arc<JobExecutor>` → `Arc<dyn JobDispatch>` via `JobExecutor::into_dispatcher`.

use agent_core::{JobBackedProvider, capabilities::manifest::{ToolDef, ToolKind, ToolManifest}};
use jobs::JobExecutor;
use serde_json::json;
use std::sync::Arc;

/// Build the `transcribe-video` capability provider wired to the given job executor.
pub fn transcribe_video_provider(executor: &Arc<JobExecutor>) -> JobBackedProvider {
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
        description: "Transcribes an audio or video file stored in RustFS via the Whisper API. Returns a task_id immediately; poll GET /v1/tasks/{id} for the result.".into(),
        kind: ToolKind::Native,
        tools: vec![ToolDef {
            name: "transcribe".into(),
            description: "Enqueue a video/audio transcription job. Returns task_id and queued status.".into(),
            input_schema,
        }],
        config: serde_json::Value::Null,
        tags: vec!["audio".into(), "video".into(), "transcription".into()],
        namespace: Some("convert.audio_to_text".into()),
        chain: None,
        tenant_scope: vec![],
        enabled: true,
        search_keywords: vec![],
        schema_version: "2.0".into(),
        category: Some("convert".into()),
        accepts: vec![],
        emits: vec![],
        idempotent: true,
        cost_hint: None,
        requires: vec![],
    };

    JobBackedProvider::new(
        manifest,
        executor.into_dispatcher(),
        "video-transcription",
        "transcribe",
    )
}
