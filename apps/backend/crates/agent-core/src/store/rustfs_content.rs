//! RustFsContentStore — thin `WorkspaceContentStore` adapter over `TenantStorageFactory`.
//!
//! Step 3.4 migration: dual-read / dual-write using stable `nodes/{node_id}/content` keys.
//! Keys starting with `"nodes/"` are routed to `TenantStorage::get/put/delete_stable_content`.
//! All other keys are treated as virtual paths and routed to the workspace object API.

use crate::store::tenant_storage::{StorageError, TenantStorageFactory, VirtualPath};
use async_trait::async_trait;
use bytes::Bytes;
use common::memory::store::WorkspaceContentStore;
use std::sync::Arc;
use tracing::{error, instrument};

pub struct RustFsContentStore {
    factory: Arc<TenantStorageFactory>,
}

impl RustFsContentStore {
    pub fn new(factory: Arc<TenantStorageFactory>) -> Arc<Self> {
        Arc::new(Self { factory })
    }
}

/// `key.starts_with("nodes/")` indicates a Step-3.4 stable key vs a legacy virtual path.
#[inline]
fn is_stable_key(key: &str) -> bool {
    key.starts_with("nodes/")
}

fn count_fallback() {
    common::metrics::workspace_content_read_fallback();
}

fn count_legacy_write_failed() {
    common::metrics::workspace_content_legacy_write_failed();
}

#[async_trait]
impl WorkspaceContentStore for RustFsContentStore {
    #[instrument(skip(self), fields(tenant_id, key))]
    async fn read(
        &self,
        tenant_id: &str,
        key: &str,
        legacy_key: Option<&str>,
    ) -> anyhow::Result<String> {
        let storage = self
            .factory
            .for_tenant(tenant_id)
            .await
            .map_err(|e| anyhow::anyhow!("storage for tenant: {e}"))?;

        if is_stable_key(key) {
            // Try stable key first.
            match storage.get_stable_content(key).await {
                Ok(bytes) => return Ok(String::from_utf8_lossy(&bytes).into_owned()),
                Err(StorageError::NotFound) => {
                    // Fall back to legacy virtual_path key if available.
                    if let Some(vp_str) = legacy_key {
                        count_fallback();
                        let vp = VirtualPath::parse(vp_str)
                            .map_err(|e| anyhow::anyhow!("invalid fallback path: {e}"))?;
                        return match storage.get_workspace_object(&vp).await {
                            Ok(bytes) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
                            Err(StorageError::NotFound) => Ok(String::new()),
                            Err(e) => Err(anyhow::anyhow!("fallback read failed: {e}")),
                        };
                    }
                    return Ok(String::new());
                }
                Err(e) => return Err(anyhow::anyhow!("stable content read failed: {e}")),
            }
        }

        // Legacy path: treat `key` as a virtual path.
        let vp =
            VirtualPath::parse(key).map_err(|e| anyhow::anyhow!("invalid virtual path: {e}"))?;
        match storage.get_workspace_object(&vp).await {
            Ok(bytes) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
            Err(StorageError::NotFound) => Ok(String::new()),
            Err(e) => Err(anyhow::anyhow!("content read failed: {e}")),
        }
    }

    #[instrument(skip(self, body), fields(tenant_id, key))]
    async fn write(
        &self,
        tenant_id: &str,
        key: &str,
        legacy_key: Option<&str>,
        body: &str,
    ) -> anyhow::Result<()> {
        let storage = self
            .factory
            .for_tenant(tenant_id)
            .await
            .map_err(|e| anyhow::anyhow!("storage for tenant: {e}"))?;

        if is_stable_key(key) {
            // Primary write to stable key — failure fails the request.
            storage
                .put_stable_content(key, Bytes::from(body.to_owned()), "text/markdown")
                .await
                .map_err(|e| anyhow::anyhow!("stable content write failed: {e}"))?;

            // Best-effort mirror to legacy virtual_path key.
            if let Some(vp_str) = legacy_key {
                match VirtualPath::parse(vp_str) {
                    Ok(vp) => {
                        if let Err(e) = storage
                            .put_workspace_object(
                                &vp,
                                Bytes::from(body.to_owned()),
                                "text/markdown",
                            )
                            .await
                        {
                            count_legacy_write_failed();
                            error!(
                                key,
                                legacy_key = vp_str,
                                error = %e,
                                "dual-write: legacy mirror write failed (primary succeeded)"
                            );
                        }
                    }
                    Err(e) => {
                        count_legacy_write_failed();
                        error!(
                            legacy_key = vp_str,
                            error = %e,
                            "dual-write: invalid legacy path, mirror skipped"
                        );
                    }
                }
            }
            return Ok(());
        }

        // Legacy path: treat `key` as a virtual path.
        let vp =
            VirtualPath::parse(key).map_err(|e| anyhow::anyhow!("invalid virtual path: {e}"))?;
        storage
            .put_workspace_object(&vp, Bytes::from(body.to_owned()), "text/markdown")
            .await
            .map(|_| ())
            .map_err(|e| anyhow::anyhow!("content write failed: {e}"))
    }

    #[instrument(skip(self), fields(tenant_id, key))]
    async fn delete(
        &self,
        tenant_id: &str,
        key: &str,
        legacy_key: Option<&str>,
    ) -> anyhow::Result<()> {
        let storage = self
            .factory
            .for_tenant(tenant_id)
            .await
            .map_err(|e| anyhow::anyhow!("storage for tenant: {e}"))?;

        if is_stable_key(key) {
            storage
                .delete_stable_content(key)
                .await
                .map_err(|e| anyhow::anyhow!("stable content delete failed: {e}"))?;
            // Best-effort legacy delete.
            if let Some(vp_str) = legacy_key
                && let Ok(vp) = VirtualPath::parse(vp_str)
            {
                let _ = storage.delete_workspace_object(&vp).await;
            }
            return Ok(());
        }

        let vp =
            VirtualPath::parse(key).map_err(|e| anyhow::anyhow!("invalid virtual path: {e}"))?;
        storage
            .delete_workspace_object(&vp)
            .await
            .map_err(|e| anyhow::anyhow!("content delete failed: {e}"))
    }
}
