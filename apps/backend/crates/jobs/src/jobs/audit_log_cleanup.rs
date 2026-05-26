//! `AuditLogCleanupJob` — deletes audit entries older than the configured retention window.
//!
//! Default retention: 90 days.  Override via `AUDIT_RETENTION_DAYS` env var.

use crate::context::JobContext;
use crate::job::ScheduledJob;
use async_trait::async_trait;
use chrono::Utc;
use std::sync::Arc;
use tracing::info;

pub struct AuditLogCleanupJob;

#[async_trait]
impl ScheduledJob for AuditLogCleanupJob {
    fn name(&self) -> &str {
        "audit-log-cleanup"
    }

    fn cron(&self) -> &str {
        "0 0 2 * * *"
    }

    async fn run(&self, ctx: Arc<JobContext>) -> anyhow::Result<()> {
        let retention_days: i64 = std::env::var("AUDIT_RETENTION_DAYS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(90);

        let before = Utc::now() - chrono::Duration::days(retention_days);
        let deleted = ctx.audit_store.prune_before(before).await.unwrap_or(0);

        info!(
            retention_days,
            deleted, "audit-log-cleanup: deleted old audit events"
        );
        Ok(())
    }
}
