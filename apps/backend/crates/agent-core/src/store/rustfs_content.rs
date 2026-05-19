//! RustFsContentStore — thin `WorkspaceContentStore` adapter over `TenantStorageFactory`.
//!
//! All key construction is delegated to `TenantStorage`; no raw `tenants/{id}` strings here.

use crate::store::tenant_storage::{TenantStorageFactory, VirtualPath};
use async_trait::async_trait;
use bytes::Bytes;
use common::memory::store::WorkspaceContentStore;
use std::sync::Arc;
use tracing::instrument;

pub struct RustFsContentStore {
    factory: Arc<TenantStorageFactory>,
}

impl RustFsContentStore {
    pub fn new(factory: Arc<TenantStorageFactory>) -> Arc<Self> {
        Arc::new(Self { factory })
    }
}

#[async_trait]
impl WorkspaceContentStore for RustFsContentStore {
    #[instrument(skip(self), fields(tenant_id, virtual_path))]
    async fn read(&self, tenant_id: &str, virtual_path: &str) -> anyhow::Result<String> {
        let vp = VirtualPath::parse(virtual_path)
            .map_err(|e| anyhow::anyhow!("invalid virtual path: {e}"))?;
        let storage = self
            .factory
            .for_tenant(tenant_id)
            .await
            .map_err(|e| anyhow::anyhow!("storage for tenant: {e}"))?;
        match storage.get_workspace_object(&vp).await {
            Ok(bytes) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
            Err(crate::store::tenant_storage::StorageError::NotFound) => Ok(String::new()),
            Err(e) => Err(anyhow::anyhow!("content read failed: {e}")),
        }
    }

    #[instrument(skip(self, body), fields(tenant_id, virtual_path))]
    async fn write(&self, tenant_id: &str, virtual_path: &str, body: &str) -> anyhow::Result<()> {
        let vp = VirtualPath::parse(virtual_path)
            .map_err(|e| anyhow::anyhow!("invalid virtual path: {e}"))?;
        let storage = self
            .factory
            .for_tenant(tenant_id)
            .await
            .map_err(|e| anyhow::anyhow!("storage for tenant: {e}"))?;
        storage
            .put_workspace_object(&vp, Bytes::from(body.to_owned()), "text/markdown")
            .await
            .map(|_| ())
            .map_err(|e| anyhow::anyhow!("content write failed: {e}"))
    }

    #[instrument(skip(self), fields(tenant_id, virtual_path))]
    async fn delete(&self, tenant_id: &str, virtual_path: &str) -> anyhow::Result<()> {
        let vp = VirtualPath::parse(virtual_path)
            .map_err(|e| anyhow::anyhow!("invalid virtual path: {e}"))?;
        let storage = self
            .factory
            .for_tenant(tenant_id)
            .await
            .map_err(|e| anyhow::anyhow!("storage for tenant: {e}"))?;
        storage
            .delete_workspace_object(&vp)
            .await
            .map_err(|e| anyhow::anyhow!("content delete failed: {e}"))
    }
}
