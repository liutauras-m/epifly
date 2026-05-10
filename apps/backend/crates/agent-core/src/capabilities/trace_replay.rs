//! `TraceReplayCapability` — translates a recorded `SessionTrace` into a
//! deterministic replay plan via `DynamicPromptCapability`.
//!
//! Ships **dry-run only** in v0.4.0: returns the plan as a JSON artifact.
//! Live replay is deferred to v0.4.1.

use crate::capabilities::card::CapabilityCard;
use crate::capabilities::manifest::{ToolKind, ToolManifest};
use crate::capabilities::provider::{CapabilityFactory, CapabilityProvider};
use crate::chains::dynamic_prompt::DynamicPromptCapability;
use crate::context::tenant::TenantContext;
use crate::llm::LlmRegistry;
use async_trait::async_trait;
use common::trace::{SessionTrace, TraceSource};
use serde_json::Value;
use sqlx::PgPool;
use std::sync::Arc;

/// Loads a `SessionTrace` from the workspace node store.
pub struct WorkspaceNodeTraceSource {
    pool: PgPool,
}

impl WorkspaceNodeTraceSource {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TraceSource for WorkspaceNodeTraceSource {
    async fn load(&self, trace_node_id: &str) -> anyhow::Result<SessionTrace> {
        let uuid = trace_node_id
            .parse::<uuid::Uuid>()
            .map_err(|_| anyhow::anyhow!("invalid trace_node_id"))?;

        let row: Option<(Option<Vec<u8>>,)> = sqlx::query_as(
            "SELECT content FROM workspace_nodes WHERE id = $1"
        )
        .bind(uuid)
        .fetch_optional(&self.pool)
        .await?;

        let (content,) = row.ok_or_else(|| anyhow::anyhow!("trace node {trace_node_id} not found"))?;
        let content = content.unwrap_or_default();
        Ok(serde_json::from_slice(&content)?)
    }
}

pub struct TraceReplayCapability {
    manifest: ToolManifest,
    inner: Arc<DynamicPromptCapability>,
    trace_source: Arc<dyn TraceSource>,
}

impl TraceReplayCapability {
    pub fn new(
        manifest: ToolManifest,
        inner: Arc<DynamicPromptCapability>,
        trace_source: Arc<dyn TraceSource>,
    ) -> Self {
        Self {
            manifest,
            inner,
            trace_source,
        }
    }
}

#[async_trait]
impl CapabilityProvider for TraceReplayCapability {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        if tool_name != "replay_session" {
            anyhow::bail!("unknown tool: {tool_name}");
        }

        let trace_node_id = input["trace_node_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("trace_node_id required"))?;
        let dry_run = input["dry_run"].as_bool().unwrap_or(true);

        let trace = self.trace_source.load(trace_node_id).await?;
        let steps_json = serde_json::to_string_pretty(&trace.steps)?;

        let prompt = format!(
            "Translate these recorded browser steps into a deterministic replay plan.\n\
             Return a JSON array of actions. Each action: {{ step, action, selector, value }}.\n\
             Steps:\n{steps_json}"
        );

        let plan_text = self
            .inner
            .invoke("dynamic_prompt", &serde_json::json!({ "prompt": prompt }), tenant)
            .await?;

        Ok(serde_json::json!({
            "trace_id": trace.id,
            "dry_run": dry_run,
            "plan": plan_text,
            "step_count": trace.steps.len(),
            "urls": trace.urls,
        }))
    }
}

pub struct TraceReplayFactory {
    llm: Arc<LlmRegistry>,
    pool: PgPool,
}

impl TraceReplayFactory {
    pub fn new(llm: Arc<LlmRegistry>, pool: PgPool) -> Self {
        Self { llm, pool }
    }
}

impl CapabilityFactory for TraceReplayFactory {
    fn supports(&self, _kind: &ToolKind, name: &str) -> bool {
        name == "trace.replay"
    }

    fn create(&self, card: CapabilityCard) -> anyhow::Result<Arc<dyn CapabilityProvider>> {
        let inner = DynamicPromptCapability::new(
            card.manifest.clone(),
            Arc::clone(&self.llm),
            self.pool.clone(),
        );
        let source: Arc<dyn TraceSource> =
            Arc::new(WorkspaceNodeTraceSource::new(self.pool.clone()));
        Ok(Arc::new(TraceReplayCapability::new(
            card.manifest,
            Arc::new(inner),
            source,
        )))
    }
}
