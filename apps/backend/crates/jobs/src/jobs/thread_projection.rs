//! `ThreadProjectionJob` — durable thread → workspace-node Markdown projection.
//!
//! Replaces the fire-and-forget `spawn_index_job` in `agent::persistence`.
//! Mirrors the `WorkspaceIndexJob` shape so we have one job pattern, not two.
//!
//! ## Coalescing
//!
//! At most one job runs per `(tenant_id, thread_id)` at a time. Concurrent
//! "assistant done" events bump a dirty flag instead of spawning another job.
//! The running worker re-runs once after marking a projection complete if the
//! dirty flag was set during its run.

use crate::context::JobContext;
use crate::job::BackgroundJob;
use agent_core::store::{ProjectionStatus, ThreadProjection};
use async_trait::async_trait;
use dashmap::DashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::info;

// ── Coalescer ─────────────────────────────────────────────────────────────────

/// Per-`(tenant_id, thread_id)` dirty-flag registry that enforces at-most-one
/// pending/running job.
///
/// Entry present → a job is pending or running.
/// Entry absent  → no job active; safe to enqueue.
pub struct ProjectionCoalescer {
    active: DashMap<(String, String), Arc<AtomicBool>>,
}

impl ProjectionCoalescer {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            active: DashMap::new(),
        })
    }

    /// Try to claim a slot for `(tenant_id, thread_id)`.
    ///
    /// Returns `Some(dirty_flag)` when no job was active (caller should enqueue).
    /// Returns `None` when a job is already active (dirty flag bumped, caller must not re-enqueue).
    pub fn try_claim(&self, tenant_id: &str, thread_id: &str) -> Option<Arc<AtomicBool>> {
        let key = (tenant_id.to_owned(), thread_id.to_owned());
        match self.active.entry(key) {
            dashmap::Entry::Occupied(e) => {
                e.get().store(true, Ordering::Release);
                common::metrics::thread_projection_coalesced();
                None
            }
            dashmap::Entry::Vacant(v) => {
                let dirty = Arc::new(AtomicBool::new(false));
                v.insert(Arc::clone(&dirty));
                Some(dirty)
            }
        }
    }

    /// Release the slot once the job finishes (success or terminal failure).
    pub fn release(&self, tenant_id: &str, thread_id: &str) {
        self.active
            .remove(&(tenant_id.to_owned(), thread_id.to_owned()));
    }
}

impl Default for ProjectionCoalescer {
    fn default() -> Self {
        Self {
            active: DashMap::new(),
        }
    }
}

// ── Input ─────────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ThreadProjectionInput {
    pub tenant_id: String,
    pub thread_id: String,
    pub reason: ProjectionReason,
    /// Initial folder where the projected node should live.
    /// Ignored if a projection row already exists (rename-preservation).
    #[serde(default)]
    pub folder_path: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionReason {
    AssistantDone,
    ManualReproject,
    Backfill,
}

// ── Job ───────────────────────────────────────────────────────────────────────

pub struct ThreadProjectionJob;

impl ThreadProjectionJob {
    pub const NAME: &'static str = "thread-projection";
}

#[async_trait]
impl BackgroundJob for ThreadProjectionJob {
    fn name(&self) -> &str {
        Self::NAME
    }

    async fn run(
        &self,
        input: serde_json::Value,
        ctx: Arc<JobContext>,
    ) -> anyhow::Result<serde_json::Value> {
        let payload: ThreadProjectionInput = serde_json::from_value(input)?;

        let projection_store = ctx.thread_projection_store.as_ref().ok_or_else(|| {
            anyhow::anyhow!("thread_projection_store not configured in JobContext")
        })?;

        let thread_store = ctx
            .thread_store
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("thread_store not configured in JobContext"))?;

        let workspace_store = ctx
            .workspace_store
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("workspace_store not configured in JobContext"))?;

        let workspace_content = ctx
            .workspace_content
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("workspace_content not configured in JobContext"))?;

        let coalescer = ctx.projection_coalescer.as_ref();

        let folder = payload
            .folder_path
            .as_deref()
            .unwrap_or("Conversations")
            .to_owned();

        let mut skipped = 0u32;
        let mut projected = 0u32;

        loop {
            // Resolve or create the projection row (Step 5.2 lookup rules).
            let mut proj = projection_store
                .resolve_or_create(&payload.tenant_id, &payload.thread_id, &folder)
                .await?;

            // Step 5.3.3: if paused, honour the pause and stop.
            if proj.status == ProjectionStatus::Paused {
                info!(
                    tenant_id = %payload.tenant_id,
                    thread_id = %payload.thread_id,
                    "thread_projection: paused — skipping"
                );
                break;
            }

            // Load thread messages (always latest state, never snapshot from enqueue).
            let messages = thread_store
                .messages(&payload.tenant_id, &payload.thread_id)
                .await
                .unwrap_or_default();

            let message_count = messages.len() as u32;
            let last_seq = messages.iter().map(|m| m.seq as u64).max().unwrap_or(0);

            // Render Markdown body (redaction applied by ProjectionRedactor in Step 5.4).
            let md_body = render_markdown(&messages);

            // Content-hash check: skip if unchanged (no Qdrant churn).
            let new_hash = compute_hash(&md_body);
            if !proj.content_hash.is_empty() && new_hash == proj.content_hash {
                info!(
                    tenant_id = %payload.tenant_id,
                    thread_id = %payload.thread_id,
                    "thread_projection: content unchanged — skipping"
                );
                common::metrics::thread_projection_skipped_unchanged();
                skipped += 1;

                // Check dirty flag before exiting.
                if should_rerun(coalescer, &payload.tenant_id, &payload.thread_id) {
                    continue;
                }
                break;
            }

            // Write the projected Markdown into the workspace content store.
            let virtual_path = format!("{}/{}.md", proj.folder_path, proj.thread_id);
            let node_id_str = proj.node_id.to_string();

            // Ensure the workspace node exists with the correct semantic_kind = Thread.
            ensure_thread_node(
                workspace_store.as_ref(),
                workspace_content.as_ref(),
                &payload.tenant_id,
                &proj,
                &virtual_path,
                &md_body,
            )
            .await?;

            // Update the projection row.
            proj.last_seq = last_seq;
            proj.content_hash = new_hash;
            proj.message_count = message_count;
            proj.projected_at = chrono::Utc::now();
            proj.status = ProjectionStatus::Active;
            proj.last_error = None;
            projection_store.upsert(&proj).await?;

            projected += 1;
            info!(
                tenant_id = %payload.tenant_id,
                thread_id = %payload.thread_id,
                node_id = %node_id_str,
                "thread_projection: projected"
            );

            // Check dirty flag — re-run if set (handles concurrent assistant turns).
            if should_rerun(coalescer, &payload.tenant_id, &payload.thread_id) {
                continue;
            }
            break;
        }

        // Release coalescer slot.
        if let Some(c) = coalescer {
            c.release(&payload.tenant_id, &payload.thread_id);
        }

        Ok(serde_json::json!({
            "projected": projected,
            "skipped_unchanged": skipped,
        }))
    }
}

/// Check and clear the dirty flag. Returns `true` if the job should re-run.
fn should_rerun(
    coalescer: Option<&Arc<ProjectionCoalescer>>,
    tenant_id: &str,
    thread_id: &str,
) -> bool {
    let Some(c) = coalescer else { return false };
    let key = (tenant_id.to_owned(), thread_id.to_owned());
    if let Some(flag) = c.active.get(&key)
        && flag
            .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    {
        return true;
    }
    false
}

/// Render thread messages to a Markdown body.
/// Redaction is applied here (Step 5.4 expands this with `ProjectionRedactor`).
/// Render thread messages to a redacted Markdown body via `DefaultProjectionRedactor`.
fn render_markdown(messages: &[common::memory::thread::Message]) -> String {
    use agent_core::projection::{
        DefaultProjectionRedactor, MessageKind, ProjectionRedactor, RenderedMessage,
    };
    let redactor = DefaultProjectionRedactor::new();
    let rendered: Vec<RenderedMessage> = messages
        .iter()
        .map(|m| RenderedMessage {
            role: m.role.clone(),
            kind: MessageKind::Text(m.content.clone()),
        })
        .collect();
    redactor.render(&rendered).into_string()
}

fn compute_hash(body: &str) -> String {
    let hash = blake3::hash(body.as_bytes());
    hex::encode(hash.as_bytes())
}

/// Ensure a `Thread`-kind workspace node exists and write the projected content.
async fn ensure_thread_node(
    workspace_store: &dyn common::memory::store::WorkspaceStore,
    workspace_content: &dyn common::memory::store::WorkspaceContentStore,
    tenant_id: &str,
    proj: &ThreadProjection,
    virtual_path: &str,
    md_body: &str,
) -> anyhow::Result<()> {
    use common::memory::workspace::WorkspaceNodeKind;

    let node_id = proj.node_id;
    let node_id_str = node_id.to_string();

    // Try to fetch existing node.
    let existing = workspace_store
        .get_accessible_node(tenant_id, "__system__", node_id)
        .await;

    match existing {
        Ok(node) => {
            // Node exists — write content with stable key.
            let (content_key, legacy_key) = match &node.object_key {
                Some(ok) => (ok.as_str().to_owned(), Some(node.virtual_path.clone())),
                None => (node.virtual_path.clone(), None),
            };
            workspace_content
                .write(tenant_id, &content_key, legacy_key.as_deref(), md_body)
                .await
                .map_err(|e| anyhow::anyhow!("write projected content: {e}"))?;
        }
        Err(_) => {
            // Node does not exist — create it with semantic_kind = Thread.
            let stable_key = format!("nodes/{node_id_str}/content");
            let mut node = common::memory::workspace::WorkspaceNode::new_conversation(
                tenant_id,
                "__system__",
                None,
                format!("{}.md", proj.thread_id),
                virtual_path,
            );
            // Override id with the deterministic one, set semantic_kind.
            node.id = node_id;
            node.object_key = Some(stable_key.clone());
            node.semantic_kind = WorkspaceNodeKind::Thread;
            node.source_type = Some("thread_projection".to_owned());
            node.source_id = Some(proj.thread_id.clone());

            // Save the node.
            let _ = workspace_store.upsert_node(node).await;

            workspace_content
                .write(tenant_id, &stable_key, Some(virtual_path), md_body)
                .await
                .map_err(|e| anyhow::anyhow!("write new projected content: {e}"))?;
        }
    }
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::JobContext;
    use crate::job::BackgroundJob;
    use agent_core::store::{ProjectionStatus, ThreadProjectionStore as _};
    use common::memory::{
        InMemoryAuditStore, InMemoryThreadStore, InMemoryWorkspaceContent, InMemoryWorkspaceStore,
    };
    use std::sync::atomic::Ordering;

    // ── Coalescer ─────────────────────────────────────────────────────────────

    #[test]
    fn coalescer_first_claim_succeeds_second_is_coalesced() {
        let coalescer = ProjectionCoalescer::default();
        let dirty = coalescer.try_claim("acme", "t1");
        assert!(dirty.is_some(), "first claim should succeed");

        let result2 = coalescer.try_claim("acme", "t1");
        assert!(
            result2.is_none(),
            "second claim on same slot should be coalesced"
        );

        let dirty_flag = dirty.unwrap();
        assert!(
            dirty_flag.load(Ordering::Acquire),
            "dirty flag must be set by the coalesced claim"
        );
    }

    #[test]
    fn coalescer_release_allows_new_claim() {
        let coalescer = ProjectionCoalescer::default();
        let _ = coalescer.try_claim("acme", "t1");
        coalescer.release("acme", "t1");
        assert!(
            coalescer.try_claim("acme", "t1").is_some(),
            "claim after release should succeed"
        );
    }

    #[test]
    fn coalescer_different_threads_are_independent() {
        let coalescer = ProjectionCoalescer::default();
        let _ = coalescer.try_claim("acme", "t1");
        assert!(
            coalescer.try_claim("acme", "t2").is_some(),
            "different thread_id gets its own independent slot"
        );
    }

    // ── ThreadProjectionJob: paused skip ─────────────────────────────────────

    fn projection_job_ctx(
        thread_store: std::sync::Arc<dyn common::memory::store::ThreadStore>,
        workspace_store: std::sync::Arc<dyn common::memory::store::WorkspaceStore>,
        workspace_content: std::sync::Arc<dyn common::memory::store::WorkspaceContentStore>,
        proj_store: std::sync::Arc<dyn agent_core::store::ThreadProjectionStore>,
        coalescer: std::sync::Arc<ProjectionCoalescer>,
    ) -> std::sync::Arc<JobContext> {
        let audit: std::sync::Arc<dyn common::audit::AuditStore> =
            std::sync::Arc::new(InMemoryAuditStore::new());
        std::sync::Arc::new(JobContext {
            audit_store: audit,
            thread_store: Some(thread_store),
            workspace_store: Some(workspace_store),
            workspace_content: Some(workspace_content),
            thread_projection_store: Some(proj_store),
            projection_coalescer: Some(coalescer),
            s3_endpoint: None,
            bucket: None,
            billing: None,
            rustfs_admin: None,
            cred_store: None,
            tenant_storage_factory: None,
            embedding_service: None,
            vector_store: None,
        })
    }

    #[tokio::test]
    async fn paused_projection_skips_without_creating_node() {
        let proj_store = agent_core::InMemoryThreadProjectionStore::new();
        proj_store
            .resolve_or_create("acme", "t1", "Conversations")
            .await
            .unwrap();
        proj_store
            .set_status("acme", "t1", ProjectionStatus::Paused)
            .await
            .unwrap();

        let workspace_store: Arc<dyn common::memory::store::WorkspaceStore> =
            Arc::new(InMemoryWorkspaceStore::new());
        let workspace_content: Arc<dyn common::memory::store::WorkspaceContentStore> =
            Arc::new(InMemoryWorkspaceContent::new());
        let thread_store: Arc<dyn common::memory::store::ThreadStore> =
            Arc::new(InMemoryThreadStore::new());
        let coalescer = ProjectionCoalescer::new();

        let ctx = projection_job_ctx(
            thread_store,
            Arc::clone(&workspace_store),
            workspace_content,
            proj_store.clone(),
            coalescer,
        );

        let input = serde_json::to_value(ThreadProjectionInput {
            tenant_id: "acme".into(),
            thread_id: "t1".into(),
            reason: ProjectionReason::AssistantDone,
            folder_path: None,
        })
        .unwrap();

        let result = ThreadProjectionJob.run(input, ctx).await.unwrap();
        assert_eq!(
            result["projected"], 0,
            "paused projection must not write anything"
        );

        // Workspace should have no nodes (paused job skips ensure_thread_node).
        let nodes = workspace_store
            .list_accessible_children("acme", "__system__", None)
            .await
            .unwrap();
        assert!(
            nodes.is_empty(),
            "no thread node should be created when paused"
        );
    }

    #[test]
    fn two_concurrent_done_events_coalesce_to_one_projection() {
        let coalescer = ProjectionCoalescer::default();

        // Simulate two concurrent done events: first claims the slot, second is coalesced.
        let dirty = coalescer.try_claim("acme", "t1");
        assert!(dirty.is_some(), "first done event claims the slot");

        let second = coalescer.try_claim("acme", "t1");
        assert!(
            second.is_none(),
            "second done event must be coalesced (not double-enqueued)"
        );

        // Verify dirty flag was set so the running job knows to re-run once.
        assert!(
            dirty.unwrap().load(Ordering::Acquire),
            "dirty flag must be set so the running job reruns once"
        );

        // Release the slot (job finished).
        coalescer.release("acme", "t1");

        // After release, new events can claim again.
        assert!(
            coalescer.try_claim("acme", "t1").is_some(),
            "after release, new events can claim"
        );
    }
}
