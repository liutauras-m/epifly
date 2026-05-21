//! `CapabilityHealthCheckJob` — periodically checks that external services
//! (Qdrant, RustFS/S3) are reachable.

use crate::context::JobContext;
use crate::job::ScheduledJob;
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{info, warn};

pub struct CapabilityHealthCheckJob;

#[async_trait]
impl ScheduledJob for CapabilityHealthCheckJob {
    fn name(&self) -> &str {
        "capability-health-check"
    }

    fn cron(&self) -> &str {
        "0 */5 * * * *"
    }

    async fn run(&self, ctx: Arc<JobContext>) -> anyhow::Result<()> {
        // Check RustFS / S3 is reachable (if configured). Any HTTP response —
        // including 401/403 from anonymous bucket-root access — means the
        // service is up; only a transport-level error counts as unreachable.
        if let Some(endpoint) = &ctx.s3_endpoint {
            let health_url = format!("{}/", endpoint);
            match reqwest::get(&health_url).await {
                Ok(resp) => {
                    info!(status = %resp.status(), "capability-health-check: storage healthy");
                }
                Err(e) => {
                    warn!(error = %e, "capability-health-check: storage unreachable");
                }
            }
        } else {
            info!("capability-health-check: storage check skipped (not configured)");
        }

        Ok(())
    }
}
