use crate::state::AppState;
use common::error::HttpError;
use common::memory::{store::DeletePlanNode, workspace::NodeKind};
use std::future::Future;
use std::sync::Arc;

/// Double-checked locking for idempotent root-folder provisioning.
/// Returns `true` if provisioning was performed this call, `false` if already seeded.
pub(super) async fn maybe_provision_root_listing<
    CheckSeeded,
    CheckSeededFuture,
    Provision,
    ProvisionFuture,
>(
    state: &AppState,
    tenant_id: &str,
    mut is_tenant_seeded: CheckSeeded,
    provision: Provision,
) -> Result<bool, HttpError>
where
    CheckSeeded: FnMut() -> CheckSeededFuture,
    CheckSeededFuture: Future<Output = bool>,
    Provision: FnOnce() -> ProvisionFuture,
    ProvisionFuture: Future<Output = Result<(), HttpError>>,
{
    let is_seeded = is_tenant_seeded().await;
    if is_seeded {
        return Ok(false);
    }

    let guard = state
        .onboarding_guards
        .entry(tenant_id.to_owned())
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone();
    let _lock = guard.lock().await;

    let still_seeded = is_tenant_seeded().await;
    if still_seeded {
        return Ok(false);
    }

    provision().await?;
    Ok(true)
}

/// Best-effort vector + content cleanup for every node in a delete plan.
/// Uses `tokio::join!` (not `try_join!`) so both cleanups run even if one fails.
/// Never propagates errors — the API response must not fail due to cleanup issues.
pub(super) async fn cleanup_after_delete(
    state: &AppState,
    tenant_id: &str,
    plan: &[DeletePlanNode],
) {
    for node in plan {
        let (content_key, legacy_key) = match &node.object_key {
            Some(ok) => (ok.as_str(), Some(node.virtual_path.as_str())),
            None => (node.virtual_path.as_str(), None),
        };
        // Folders have no content object; skip content deletion to avoid unnecessary S3 calls.
        let content_fut = async {
            if node.kind == NodeKind::Folder {
                Ok(())
            } else {
                state
                    .workspace_content
                    .delete_all_versions(tenant_id, content_key, legacy_key)
                    .await
            }
        };
        let (vec_res, content_res) = tokio::join!(
            state.vector_store.delete_by_node_id(tenant_id, node.id),
            content_fut,
        );
        if let Err(e) = vec_res {
            tracing::error!(error = %e, node_id = %node.id, "vector cleanup failed after delete");
        }
        if let Err(e) = content_res {
            tracing::error!(error = %e, node_id = %node.id, "content cleanup failed after delete");
        }
    }
}
