//! `JobSchedulerService` — wraps `tokio-cron-scheduler` and drives all `ScheduledJob`s.

use crate::job::ScheduledJob;
use crate::registry::JobRegistry;
use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobScheduler};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

/// Manages the lifecycle of all scheduled (cron) jobs.
pub struct JobSchedulerService {
    inner: JobScheduler,
}

impl JobSchedulerService {
    /// Create, populate, and start the scheduler from the registry.
    ///
    /// `cancel` is propagated to each job closure via `tokio::select!` so that
    /// a running job stops at its next async yield point when the token is
    /// triggered.  A background task also calls `shutdown()` on cancellation so
    /// the scheduler itself stops accepting new runs.
    pub async fn start(registry: &JobRegistry, cancel: CancellationToken) -> anyhow::Result<Self> {
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
            let job_cancel = cancel.clone();

            let cron_job = Job::new_async(cron.as_str(), move |_uuid, _lock| {
                let ctx = Arc::clone(&ctx);
                let job = Arc::clone(&job_arc);
                let n = name.clone();
                let cancel = job_cancel.clone();
                Box::pin(async move {
                    // Don't start a new run if shutdown is in progress.
                    if cancel.is_cancelled() {
                        return;
                    }
                    info!(job = %n, "scheduled job started");
                    tokio::select! {
                        result = job.run(Arc::clone(&ctx)) => {
                            match result {
                                Ok(()) => info!(job = %n, "scheduled job completed"),
                                Err(e) => error!(job = %n, error = %e, "scheduled job failed"),
                            }
                        }
                        _ = cancel.cancelled() => {
                            warn!(job = %n, "scheduled job interrupted by shutdown");
                        }
                    }
                })
            })?;

            sched.add(cron_job).await?;
            info!(
                job = job.name(),
                cron = job.cron(),
                "registered scheduled job"
            );
        }

        sched.start().await?;
        info!("job scheduler started");

        // Spawn a background watcher that shuts down the scheduler when the
        // cancel token is triggered (e.g. on SIGTERM).
        let mut shutdown_sched = sched.clone();
        tokio::spawn(async move {
            cancel.cancelled().await;
            info!("cancellation token fired — shutting down job scheduler");
            if let Err(e) = shutdown_sched.shutdown().await {
                warn!(error = %e, "job scheduler shutdown error during cancellation");
            }
        });

        Ok(Self { inner: sched })
    }

    /// Gracefully shut down the scheduler.
    pub async fn shutdown(&mut self) {
        if let Err(e) = self.inner.shutdown().await {
            warn!(error = %e, "job scheduler shutdown error");
        }
    }
}
