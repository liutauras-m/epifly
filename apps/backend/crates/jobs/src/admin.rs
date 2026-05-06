//! `JobAdmin` — management API over the job registry and executor.

use crate::executor::JobExecutor;
use crate::job::TaskStatus;
use crate::registry::JobRegistry;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

/// Summary of a registered job (scheduled or background).
#[derive(Debug, Serialize, Deserialize)]
pub struct JobSummary {
    pub name: String,
    pub kind: JobKind,
    /// Cron expression (only for `Scheduled` jobs).
    pub cron: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum JobKind {
    Scheduled,
    Background,
}

/// Administrative facade over the job registry and executor.
pub struct JobAdmin {
    registry: Arc<JobRegistry>,
    executor: Arc<JobExecutor>,
}

impl JobAdmin {
    pub fn new(registry: Arc<JobRegistry>, executor: Arc<JobExecutor>) -> Self {
        Self { registry, executor }
    }

    /// List all registered jobs (scheduled + background).
    pub fn list_jobs(&self) -> Vec<JobSummary> {
        let mut jobs: Vec<JobSummary> = self
            .registry
            .scheduled_jobs()
            .iter()
            .map(|j| JobSummary {
                name: j.name().to_owned(),
                kind: JobKind::Scheduled,
                cron: Some(j.cron().to_owned()),
                enabled: j.enabled(),
            })
            .collect();

        for name in self.registry.background_job_names() {
            jobs.push(JobSummary {
                name,
                kind: JobKind::Background,
                cron: None,
                enabled: true,
            });
        }

        jobs
    }

    /// Get a single job summary by name.
    pub fn get_job(&self, name: &str) -> Option<JobSummary> {
        self.list_jobs().into_iter().find(|j| j.name == name)
    }

    /// Enqueue a background job immediately.
    pub async fn run_now(&self, name: &str, input: serde_json::Value) -> anyhow::Result<Uuid> {
        self.executor.enqueue(name, input).await
    }

    /// List tasks (newest first, up to `limit`).
    pub async fn list_tasks(&self, limit: usize) -> Vec<TaskStatus> {
        self.executor.list_tasks(limit).await
    }

    /// Get a single task status.
    pub async fn get_task(&self, id: Uuid) -> Option<TaskStatus> {
        self.executor.get_status(id).await
    }
}
