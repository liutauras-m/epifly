//! `WorkspaceBackfillObjectKeyJob` — Step 3.5 migration backfill.
//!
//! Scans every `WorkspaceNode` that still uses the legacy `virtual_path` key for content
//! storage (i.e. `object_key IS NULL`) and copies its content to the stable
//! `nodes/{node_id}/content` key, then sets `object_key` on the node so it is not processed
//! again.
//!
//! # Guarantees
//!
//! - **Idempotent:** nodes whose `object_key` is already set are skipped by the scan.
//!   Re-running the job on a fully-migrated store is a no-op.
//! - **Resumable:** the job marks each node as migrated (via `upsert_node`) immediately
//!   after copying its content.  If the process crashes mid-run, the next run picks up
//!   where it left off.
//! - **Best-effort on empty content:** nodes with no content at their legacy path are
//!   marked as migrated with an empty `object_key` path — they never had content to copy.
//!
//! # Operator usage
//!
//!   POST /internal/jobs/workspace-backfill-object-key/trigger
//!
//! Monitor progress with `cargo xtask audit-object-keys --db <path>`.
//!
//! # Cutover gate (plan Step 3.5)
//!
//! After the backfill completes:
//! 1. Confirm `cargo xtask audit-object-keys` reports 100% coverage.
//! 2. Confirm `workspace_content_read_fallback` metric is at 0 for 24 h.
//! 3. Remove the fallback read path; write to `node_id` key only.

use crate::context::JobContext;
use crate::job::ScheduledJob;
use async_trait::async_trait;
use common::memory::workspace::WorkspaceNode;
use std::sync::Arc;
use tracing::{error, info, warn};

pub struct WorkspaceBackfillObjectKeyJob;

#[async_trait]
impl ScheduledJob for WorkspaceBackfillObjectKeyJob {
    fn name(&self) -> &str {
        "workspace-backfill-object-key"
    }

    /// Not auto-scheduled — triggered on-demand via the internal API.
    fn cron(&self) -> &str {
        "0 0 4 31 2 *" // Feb 31 — never fires automatically
    }

    async fn run(&self, ctx: Arc<JobContext>) -> anyhow::Result<()> {
        let (Some(workspace_store), Some(workspace_content)) =
            (ctx.workspace_store.as_ref(), ctx.workspace_content.as_ref())
        else {
            info!(
                "workspace-backfill-object-key: workspace_store or workspace_content not configured — skipping"
            );
            return Ok(());
        };

        let nodes = workspace_store.scan_nodes_needing_backfill().await?;

        if nodes.is_empty() {
            info!("workspace-backfill-object-key: no nodes need migration — done");
            return Ok(());
        }

        info!(
            total = nodes.len(),
            "workspace-backfill-object-key: starting migration"
        );

        let mut migrated = 0u32;
        let mut skipped_empty = 0u32;
        let mut errors = 0u32;

        for node in nodes {
            let node_id_str = node.id.to_string();
            let new_key = format!("nodes/{node_id_str}/content");

            // Read existing content via the legacy virtual_path key.
            let content = match workspace_content
                .read(&node.tenant_id, &node.virtual_path, None)
                .await
            {
                Ok(c) => c,
                Err(e) => {
                    warn!(
                        tenant_id = %node.tenant_id,
                        node_id   = %node_id_str,
                        path      = %node.virtual_path,
                        error     = %e,
                        "backfill: failed to read legacy content — skipping node"
                    );
                    errors += 1;
                    continue;
                }
            };

            // Write to the stable node_id key (primary path; dual-write handles legacy mirror).
            if !content.is_empty() {
                if let Err(e) = workspace_content
                    .write(
                        &node.tenant_id,
                        &new_key,
                        Some(&node.virtual_path),
                        &content,
                    )
                    .await
                {
                    error!(
                        tenant_id = %node.tenant_id,
                        node_id   = %node_id_str,
                        new_key   = %new_key,
                        error     = %e,
                        "backfill: failed to write to node_id key — node not marked migrated"
                    );
                    errors += 1;
                    continue;
                }
            } else {
                skipped_empty += 1;
            }

            // Mark the node as migrated by setting object_key.
            let mut updated = node.clone();
            updated.object_key = Some(new_key.clone());
            if let Err(e) = workspace_store.upsert_node(updated).await {
                error!(
                    tenant_id = %node.tenant_id,
                    node_id   = %node_id_str,
                    error     = %e,
                    "backfill: content written but failed to set object_key — will be retried"
                );
                errors += 1;
                continue;
            }

            migrated += 1;

            if migrated.is_multiple_of(100) {
                info!(migrated, errors, "workspace-backfill-object-key: progress");
            }
        }

        info!(
            migrated,
            skipped_empty, errors, "workspace-backfill-object-key: complete"
        );

        if errors > 0 {
            anyhow::bail!(
                "workspace-backfill-object-key: {errors} node(s) failed — re-run to retry"
            );
        }

        Ok(())
    }
}

// ── Helper: snapshot for the xtask audit ────────────────────────────────────

/// Coverage report produced by scanning a workspace store.
///
/// Used by `cargo xtask audit-object-keys` and the `/internal/jobs/backfill/status`
/// endpoint to confirm readiness for the cutover gate.
#[derive(Debug)]
pub struct ObjectKeyCoverageReport {
    pub total_nodes: usize,
    pub migrated: usize,
    pub needing_backfill: usize,
    pub coverage_pct: f64,
    /// First up to 50 node IDs still needing migration (for triage).
    pub sample_fallback_node_ids: Vec<String>,
}

impl ObjectKeyCoverageReport {
    /// Build from raw node scan results.  `all_nodes` should be every node
    /// in the store; `needing_backfill` should be those without `object_key`.
    pub fn from_counts(total: usize, backfill_nodes: &[WorkspaceNode]) -> Self {
        let needing = backfill_nodes.len();
        let migrated = total.saturating_sub(needing);
        let pct = if total == 0 {
            100.0
        } else {
            (migrated as f64 / total as f64) * 100.0
        };
        let sample: Vec<String> = backfill_nodes
            .iter()
            .take(50)
            .map(|n| n.id.to_string())
            .collect();
        Self {
            total_nodes: total,
            migrated,
            needing_backfill: needing,
            coverage_pct: pct,
            sample_fallback_node_ids: sample,
        }
    }
}
