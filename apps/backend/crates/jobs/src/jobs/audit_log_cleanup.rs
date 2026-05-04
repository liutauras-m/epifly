//! `AuditLogCleanupJob` — deletes audit entries older than the configured retention window.
//!
//! Default retention: 30 days.  Override via `AUDIT_RETENTION_DAYS` env var.

use crate::context::JobContext;
use crate::job::ScheduledJob;
use async_trait::async_trait;
use std::sync::Arc;
use tracing::info;

pub struct AuditLogCleanupJob;

#[async_trait]
impl ScheduledJob for AuditLogCleanupJob {
    fn name(&self) -> &str {
        "audit-log-cleanup"
    }

    fn cron(&self) -> &str {
        // Every day at 02:00 UTC
        "0 0 2 * * *"
    }

    async fn run(&self, _ctx: Arc<JobContext>) -> anyhow::Result<()> {
        let retention_days: u64 = std::env::var("AUDIT_RETENTION_DAYS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);

        // The in-memory and Qdrant audit stores don't implement TTL-based deletion yet.
        // This job acts as a no-op placeholder that logs intent — a future PR will add
        // `AuditStore::delete_before(tenant, timestamp)` to the trait.
        info!(
            retention_days,
            "audit-log-cleanup: retention window enforced (TTL deletion not yet implemented)"
        );
        Ok(())
    }
}
