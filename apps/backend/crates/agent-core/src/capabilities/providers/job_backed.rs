//! Generic job-backed capability provider.
//!
//! Defines the `JobDispatch` trait so `agent-core` can hold job-backed providers
//! without a direct dependency on the `jobs` crate. `JobExecutor` (in `jobs`) implements
//! `JobDispatch`; the gateway wires the two together at startup.
//!
//! Canonical placement: `agent-core/src/capabilities/providers/job_backed.rs` (§0 invariant 10).

use crate::capabilities::manifest::ToolManifest;
use crate::capabilities::provider::CapabilityProvider;
use crate::context::tenant::TenantContext;
use async_trait::async_trait;
use serde_json::{Value, json};
use std::sync::Arc;

// ── JobDispatch ────────────────────────────────────────────────────────────────

/// Narrow async job-enqueue interface satisfied by `jobs::JobExecutor`.
///
/// Defined here so `agent-core` (which cannot depend on `jobs`) can host the
/// generic `JobBackedProvider`. The concrete `JobExecutor` implements this trait
/// inside the `jobs` crate; the gateway passes `Arc<dyn JobDispatch>` at construction.
#[async_trait]
pub trait JobDispatch: Send + Sync + 'static {
    /// Enqueue a background job of the given type with a JSON payload.
    /// Returns the opaque task ID (UUID string).
    async fn enqueue(&self, job_type: &str, payload: Value) -> anyhow::Result<String>;
}

// ── JobBackedProvider ──────────────────────────────────────────────────────────

/// A `CapabilityProvider` that enqueues an async background job via `JobDispatch`
/// and returns a `task_id` immediately, without waiting for completion.
///
/// Register instances at gateway startup via `CapabilityRegistry::register_provider`
/// (they need `Arc<dyn JobDispatch>` which is only available after the executor is built).
pub struct JobBackedProvider {
    manifest: ToolManifest,
    dispatcher: Arc<dyn JobDispatch>,
    /// The job type string passed to `JobDispatch::enqueue`.
    job_type: String,
    /// The single tool name this provider responds to.
    tool_name: String,
}

impl JobBackedProvider {
    pub fn new(
        manifest: ToolManifest,
        dispatcher: Arc<dyn JobDispatch>,
        job_type: impl Into<String>,
        tool_name: impl Into<String>,
    ) -> Self {
        Self {
            manifest,
            dispatcher,
            job_type: job_type.into(),
            tool_name: tool_name.into(),
        }
    }
}

#[async_trait]
impl CapabilityProvider for JobBackedProvider {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        if tool_name != self.tool_name {
            anyhow::bail!("unknown tool: {tool_name}");
        }
        let tenant_id = tenant
            .map(|t| t.tenant_id.as_str().to_owned())
            .unwrap_or_else(|| "__dev__".to_owned());

        let mut payload = input.clone();
        if let Some(obj) = payload.as_object_mut() {
            obj.insert("tenant_id".into(), json!(tenant_id));
        }

        let task_id = self.dispatcher.enqueue(&self.job_type, payload).await?;
        Ok(json!({
            "task_id": task_id,
            "status": "queued",
            "poll_url": format!("/v1/tasks/{}", task_id),
        }))
    }
}
