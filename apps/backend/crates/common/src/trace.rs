use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    Click,
    Input,
    Submit,
    Navigate,
    Scroll,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UserStep {
    pub seq: usize,
    pub kind: StepKind,
    pub selector: Option<String>,
    /// Redacted to None for password/sensitive fields.
    pub value: Option<String>,
    pub url: String,
    pub timestamp_ms: u64,
    /// PNG base64; None for sensitive regions.
    pub screenshot_base64: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SessionTrace {
    pub id: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub steps: Vec<UserStep>,
    pub urls: Vec<String>,
}

/// Implemented by the Tauri shell recorder and any future headless / mobile recorders.
pub trait SessionRecorder: Send + Sync + 'static {
    fn record_step(&self, step: UserStep);
    fn snapshot(&self) -> SessionTrace;
    fn reset(&self);
}

/// Abstracts "where the trace JSON came from" — workspace node, uploaded file, etc.
#[async_trait::async_trait]
pub trait TraceSource: Send + Sync + 'static {
    async fn load(&self, trace_node_id: &str) -> anyhow::Result<SessionTrace>;
}
