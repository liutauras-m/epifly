//! `JobRegistry` — holds all registered scheduled and background jobs together with
//! the shared `JobContext`.

use crate::context::JobContext;
use crate::job::{BackgroundJob, ScheduledJob};
use std::collections::HashMap;
use std::sync::Arc;

/// Central registry of all jobs known to the platform.
pub struct JobRegistry {
    pub(crate) scheduled: Vec<Arc<dyn ScheduledJob>>,
    pub(crate) background: HashMap<String, Arc<dyn BackgroundJob>>,
    pub(crate) ctx: Arc<JobContext>,
}

impl JobRegistry {
    pub fn new(ctx: Arc<JobContext>) -> Self {
        Self {
            scheduled: Vec::new(),
            background: HashMap::new(),
            ctx,
        }
    }

    /// Register a scheduled (cron) job.
    pub fn register_scheduled(&mut self, job: impl ScheduledJob + 'static) {
        self.scheduled.push(Arc::new(job));
    }

    /// Register a background (on-demand) job.
    pub fn register_background(&mut self, job: impl BackgroundJob + 'static) {
        let name = job.name().to_owned();
        self.background.insert(name, Arc::new(job));
    }

    /// Shared job context.
    pub fn ctx(&self) -> Arc<JobContext> {
        Arc::clone(&self.ctx)
    }

    /// All registered scheduled jobs.
    pub fn scheduled_jobs(&self) -> &[Arc<dyn ScheduledJob>] {
        &self.scheduled
    }

    /// Lookup a background job by name.
    pub fn get_background(&self, name: &str) -> Option<Arc<dyn BackgroundJob>> {
        self.background.get(name).cloned()
    }

    /// All background job names.
    pub fn background_job_names(&self) -> Vec<String> {
        self.background.keys().cloned().collect()
    }
}
