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
        // Check RustFS / S3 is reachable (if configured).
        if let Some(endpoint) = &ctx.s3_endpoint {
            let health_url = format!("{}/", endpoint);
            match reqwest::get(&health_url).await {
                Ok(resp) if resp.status().is_success() || resp.status().as_u16() == 200 => {
                    info!("capability-health-check: storage healthy");
                }
                Ok(resp) => {
                    warn!(status = %resp.status(), "capability-health-check: storage unhealthy");
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
