//! `AuditLogCleanupJob` — deletes audit entries older than the configured retention window.
//!
//! Default retention: 90 days.  Override via `AUDIT_RETENTION_DAYS` env var.

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

    async fn run(&self, ctx: Arc<JobContext>) -> anyhow::Result<()> {
        let retention_days: i64 = std::env::var("AUDIT_RETENTION_DAYS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(90);

        let Some(pool) = &ctx.pool else {
            info!("audit-log-cleanup: skipped (no postgres pool in test mode)");
            return Ok(());
        };

        let result = sqlx::query!(
            "DELETE FROM audit_events WHERE timestamp < now() - ($1 || ' days')::interval",
            retention_days.to_string(),
        )
        .execute(pool)
        .await?;

        info!(
            retention_days,
            deleted = result.rows_affected(),
            "audit-log-cleanup: deleted old audit events"
        );
        Ok(())
    }
}
