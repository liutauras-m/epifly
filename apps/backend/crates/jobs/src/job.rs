//! Core job traits: `ScheduledJob` (cron-driven) and `BackgroundJob` (on-demand async tasks).

use crate::context::JobContext;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

// ── Task lifecycle ────────────────────────────────────────────────────────────

/// Current execution state of a background task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskState {
    Queued,
    Running,
    Completed,
    Failed,
}

impl std::fmt::Display for TaskState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskState::Queued => write!(f, "queued"),
            TaskState::Running => write!(f, "running"),
            TaskState::Completed => write!(f, "completed"),
            TaskState::Failed => write!(f, "failed"),
        }
    }
}

/// Snapshot of a background task — returned by `GET /v1/tasks/{id}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStatus {
    pub id: Uuid,
    pub job_name: String,
    pub state: TaskState,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// JSON result payload (present when `state == Completed`).
    pub result: Option<serde_json::Value>,
    /// Human-readable error (present when `state == Failed`).
    pub error: Option<String>,
}

impl TaskStatus {
    pub fn new(id: Uuid, job_name: impl Into<String>) -> Self {
        let now = chrono::Utc::now();
        Self {
            id,
            job_name: job_name.into(),
            state: TaskState::Queued,
            created_at: now,
            updated_at: now,
            result: None,
            error: None,
        }
    }
}

// ── Scheduled jobs ────────────────────────────────────────────────────────────

/// A job driven by a cron expression — registered in `JobSchedulerService`.
#[async_trait]
pub trait ScheduledJob: Send + Sync + 'static {
    /// Unique slug (e.g. `capability-health-check`).
    fn name(&self) -> &str;
    /// Standard cron expression (6 fields, e.g. `"0 */5 * * * *"` = every 5 min).
    fn cron(&self) -> &str;
    /// Whether this job is active. Default `true`.
    fn enabled(&self) -> bool {
        true
    }
    /// Execute one run.
    async fn run(&self, ctx: Arc<JobContext>) -> anyhow::Result<()>;
}

// ── Background jobs ───────────────────────────────────────────────────────────

/// A job that can be enqueued on demand, runs asynchronously, and produces a `TaskStatus`.
#[async_trait]
pub trait BackgroundJob: Send + Sync + 'static {
    /// Unique slug (e.g. `video-transcription`).
    fn name(&self) -> &str;
    /// Execute the job with the given JSON `input` payload.
    async fn run(
        &self,
        input: serde_json::Value,
        ctx: Arc<JobContext>,
    ) -> anyhow::Result<serde_json::Value>;
}
