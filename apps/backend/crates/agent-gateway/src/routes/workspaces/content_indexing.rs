use crate::state::AppState;
use jobs::jobs::{WorkspaceIndexInput, WorkspaceIndexJob};
use ulid::Ulid;

/// Enqueue a durable `WorkspaceIndexJob` for the given node.
///
/// The job reads content from the store, chunks it, embeds each chunk, and
/// upserts into Qdrant. A `content_version` guard prevents stale upserts when
/// content changes while the job is queued.
pub(super) async fn enqueue_reindex(
    state: &AppState,
    tenant_id: String,
    node_id: Ulid,
    content_version: i64,
) {
    let input = serde_json::to_value(WorkspaceIndexInput {
        tenant_id,
        node_id: node_id.to_string(),
        content_version,
    })
    .expect("WorkspaceIndexInput is serializable");

    if let Err(e) = state
        .job_executor
        .enqueue(WorkspaceIndexJob::NAME, input)
        .await
    {
        tracing::warn!(
            node_id = %node_id,
            error = %e,
            "failed to enqueue WorkspaceIndexJob"
        );
    }
}
