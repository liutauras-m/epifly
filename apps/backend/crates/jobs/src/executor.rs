//! `JobExecutor` — enqueues `BackgroundJob`s, tracks `TaskStatus`, and exposes SSE streams.

use crate::job::{TaskState, TaskStatus};
use crate::registry::JobRegistry;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::sync::broadcast;
use tracing::{error, info};
use uuid::Uuid;

/// Event published on the per-task SSE channel.
#[derive(Debug, Clone)]
pub struct TaskEvent {
    pub task_id: Uuid,
    pub state: TaskState,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}

/// In-memory background task executor.
///
/// All state is held in memory — a process restart clears task history.  For
/// production persistence swap in an Apalis + Postgres backend (the `BackgroundJob`
/// trait is unchanged).
pub struct JobExecutor {
    tasks: RwLock<HashMap<Uuid, TaskStatus>>,
    /// Per-task broadcast channels (created on enqueue, dropped when complete).
    channels: RwLock<HashMap<Uuid, broadcast::Sender<TaskEvent>>>,
    registry: Arc<JobRegistry>,
}

impl JobExecutor {
    pub fn new(registry: Arc<JobRegistry>) -> Arc<Self> {
        Arc::new(Self {
            tasks: RwLock::new(HashMap::new()),
            channels: RwLock::new(HashMap::new()),
            registry,
        })
    }

    /// Enqueue a background job by name and return the new `task_id`.
    ///
    /// Returns `Err` if no job with that name is registered.
    pub async fn enqueue(
        self: &Arc<Self>,
        job_name: &str,
        input: serde_json::Value,
    ) -> anyhow::Result<Uuid> {
        let job = self
            .registry
            .get_background(job_name)
            .ok_or_else(|| anyhow::anyhow!("no background job registered: {job_name}"))?;

        let task_id = Uuid::new_v4();
        let status = TaskStatus::new(task_id, job_name);
        let job_name_owned = job_name.to_owned();

        // Create broadcast channel for SSE
        let (tx, _) = broadcast::channel::<TaskEvent>(32);
        self.tasks.write().await.insert(task_id, status);
        self.channels.write().await.insert(task_id, tx.clone());

        let executor = Arc::clone(self);
        let ctx = executor.registry.ctx();

        tokio::spawn(async move {
            // Mark running
            executor
                .update_state(task_id, TaskState::Running, None, None, &tx)
                .await;

            info!(task_id = %task_id, job = %job_name_owned, "background job started");

            match job.run(input, ctx).await {
                Ok(result) => {
                    info!(task_id = %task_id, job = %job_name_owned, "background job completed");
                    executor
                        .update_state(task_id, TaskState::Completed, Some(result), None, &tx)
                        .await;
                }
                Err(e) => {
                    error!(task_id = %task_id, job = %job_name_owned, error = %e, "background job failed");
                    executor
                        .update_state(task_id, TaskState::Failed, None, Some(e.to_string()), &tx)
                        .await;
                }
            }

            // Drop the SSE channel — subscribers will get a closed stream
            executor.channels.write().await.remove(&task_id);
        });

        Ok(task_id)
    }

    /// Get a task's current status snapshot.
    pub async fn get_status(&self, task_id: Uuid) -> Option<TaskStatus> {
        self.tasks.read().await.get(&task_id).cloned()
    }

    /// List all known task statuses (newest first, up to `limit`).
    pub async fn list_tasks(&self, limit: usize) -> Vec<TaskStatus> {
        let tasks = self.tasks.read().await;
        let mut all: Vec<TaskStatus> = tasks.values().cloned().collect();
        all.sort_by_key(|x| std::cmp::Reverse(x.created_at));
        all.truncate(limit);
        all
    }

    /// Subscribe to task events for SSE streaming.
    ///
    /// Returns `None` if the task is already complete (no channel).
    pub async fn subscribe(&self, task_id: Uuid) -> Option<broadcast::Receiver<TaskEvent>> {
        self.channels
            .read()
            .await
            .get(&task_id)
            .map(|tx| tx.subscribe())
    }

    async fn update_state(
        &self,
        task_id: Uuid,
        state: TaskState,
        result: Option<serde_json::Value>,
        error: Option<String>,
        tx: &broadcast::Sender<TaskEvent>,
    ) {
        let mut tasks = self.tasks.write().await;
        if let Some(t) = tasks.get_mut(&task_id) {
            t.state = state.clone();
            t.updated_at = chrono::Utc::now();
            t.result = result.clone();
            t.error = error.clone();
        }
        let _ = tx.send(TaskEvent {
            task_id,
            state,
            result,
            error,
        });
    }
}
