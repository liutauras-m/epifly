//! `JobSchedulerService` — wraps `tokio-cron-scheduler` and drives all `ScheduledJob`s.

use crate::job::ScheduledJob;
use crate::registry::JobRegistry;
use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{error, info, warn};

/// Manages the lifecycle of all scheduled (cron) jobs.
pub struct JobSchedulerService {
    inner: JobScheduler,
}

impl JobSchedulerService {
    /// Create, populate, and start the scheduler from the registry.
    pub async fn start(registry: &JobRegistry) -> anyhow::Result<Self> {
        let sched = JobScheduler::new().await?;

        for job in registry.scheduled_jobs() {
            if !job.enabled() {
                info!(job = job.name(), "skipping disabled scheduled job");
                continue;
            }

            let ctx = registry.ctx();
            let job_arc: Arc<dyn ScheduledJob> = Arc::clone(job);
            let name = job.name().to_owned();
            let cron = job.cron().to_owned();

            let cron_job = Job::new_async(cron.as_str(), move |_uuid, _lock| {
                let ctx = Arc::clone(&ctx);
                let job = Arc::clone(&job_arc);
                let n = name.clone();
                Box::pin(async move {
                    info!(job = %n, "scheduled job started");
                    if let Err(e) = job.run(Arc::clone(&ctx)).await {
                        error!(job = %n, error = %e, "scheduled job failed");
                    } else {
                        info!(job = %n, "scheduled job completed");
                    }
                })
            })?;

            sched.add(cron_job).await?;
            info!(job = job.name(), cron = job.cron(), "registered scheduled job");
        }

        sched.start().await?;
        info!("job scheduler started");
        Ok(Self { inner: sched })
    }

    /// Gracefully shut down the scheduler.
    pub async fn shutdown(&mut self) {
        if let Err(e) = self.inner.shutdown().await {
            warn!(error = %e, "job scheduler shutdown error");
        }
    }
}
