//! `CapabilityHealthCheckJob` — periodically checks that all registered capabilities
//! respond to a lightweight ping.

use crate::context::JobContext;
use crate::job::ScheduledJob;
use async_trait::async_trait;
use sqlx;
use std::sync::Arc;
use tracing::{info, warn};

/// Runs every 5 minutes. Logs a warning for any capability that is unreachable.
pub struct CapabilityHealthCheckJob;

#[async_trait]
impl ScheduledJob for CapabilityHealthCheckJob {
    fn name(&self) -> &str {
        "capability-health-check"
    }

    fn cron(&self) -> &str {
        // every 5 minutes
        "0 */5 * * * *"
    }

    async fn run(&self, ctx: Arc<JobContext>) -> anyhow::Result<()> {
        // Verify Postgres is reachable when configured.
        if let Some(pool) = &ctx.pool {
            match sqlx::query("SELECT 1").execute(pool).await {
                Ok(_) => {
                    info!("capability-health-check: postgres healthy");
                }
                Err(e) => {
                    warn!(error = %e, "capability-health-check: postgres unreachable");
                }
            }
        } else {
            info!("capability-health-check: postgres check skipped (test mode)");
        }

        // Verify MinIO / S3 is reachable (if configured)
        if let Some(endpoint) = &ctx.minio_endpoint {
            let health_url = format!("{}/minio/health/live", endpoint);
            match reqwest::get(&health_url).await {
                Ok(resp) if resp.status().is_success() || resp.status().as_u16() == 200 => {
                    info!("capability-health-check: minio healthy");
                }
                Ok(resp) => {
                    warn!(status = %resp.status(), "capability-health-check: minio unhealthy");
                }
                Err(e) => {
                    warn!(error = %e, "capability-health-check: minio unreachable");
                }
            }
        }

        Ok(())
    }
}
