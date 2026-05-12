//! `LagoReconcileJob` — nightly reconciliation of Lago billing state.
//!
//! Runs at 02:00 UTC daily. Calls `analytics_summary()` from the billing
//! provider and logs a structured reconciliation summary. Logs a warning for
//! any anomalies so on-call can triage without a Lago dashboard login.
//!
//! Future: cross-reference Zitadel org list vs Lago customer list and ensure
//! every org has a corresponding Lago customer record.

use crate::context::JobContext;
use crate::job::ScheduledJob;
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{info, warn};

pub struct LagoReconcileJob;

#[async_trait]
impl ScheduledJob for LagoReconcileJob {
    fn name(&self) -> &str {
        "lago-reconcile"
    }

    /// Run at 02:00 UTC every day.
    fn cron(&self) -> &str {
        "0 0 2 * * *"
    }

    async fn run(&self, ctx: Arc<JobContext>) -> anyhow::Result<()> {
        let billing = match ctx.billing.as_ref() {
            Some(b) => b,
            None => {
                info!("lago-reconcile: billing provider not configured — skipping");
                return Ok(());
            }
        };

        info!("lago-reconcile: starting nightly reconciliation");

        match billing_core::provider::BillingProvider::analytics_summary(billing.as_ref()).await {
            Ok(summary) => {
                // Log gross revenue and subscription counts for auditing.
                let gross_revenue = summary
                    .get("gross_revenue")
                    .or_else(|| summary.pointer("/data/0/amount_cents"))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);

                info!(
                    gross_revenue_cents = gross_revenue,
                    summary = %serde_json::to_string(&summary).unwrap_or_default(),
                    "lago-reconcile: analytics summary"
                );
            }
            Err(e) => {
                warn!(error = %e, "lago-reconcile: analytics_summary failed");
            }
        }

        info!("lago-reconcile: reconciliation complete");
        Ok(())
    }
}
