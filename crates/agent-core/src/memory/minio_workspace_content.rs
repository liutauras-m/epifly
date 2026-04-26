/// MinioWorkspaceContent — reads/writes .md files for Conversation nodes.
///
/// All paths are scoped under `tenants/{tenant_id}/workspaces/{virtual_path}`.
/// Uses `object_store` (S3-compatible), the same client wired into AppState.file_store.
use async_trait::async_trait;
use bytes::Bytes;
use common::memory::store::WorkspaceContentStore;
use object_store::{ObjectStore, path::Path as OsPath};
use std::sync::Arc;
use tracing::{instrument, warn};

pub struct MinioWorkspaceContent {
    store: Arc<dyn ObjectStore>,
}

impl MinioWorkspaceContent {
    pub fn new(store: Arc<dyn ObjectStore>) -> Self {
        Self { store }
    }

    fn object_key(tenant_id: &str, virtual_path: &str) -> OsPath {
        OsPath::from(format!("tenants/{tenant_id}/workspaces/{virtual_path}"))
    }
}

#[async_trait]
impl WorkspaceContentStore for MinioWorkspaceContent {
    #[instrument(skip(self), fields(tenant_id, virtual_path))]
    async fn read(&self, tenant_id: &str, virtual_path: &str) -> anyhow::Result<String> {
        let key = Self::object_key(tenant_id, virtual_path);
        match self.store.get(&key).await {
            Ok(result) => {
                let bytes = result.bytes().await?;
                Ok(String::from_utf8_lossy(&bytes).into_owned())
            }
            Err(object_store::Error::NotFound { .. }) => Ok(String::new()),
            Err(e) => Err(anyhow::anyhow!("workspace content read failed: {e}")),
        }
    }

    #[instrument(skip(self, body), fields(tenant_id, virtual_path))]
    async fn write(&self, tenant_id: &str, virtual_path: &str, body: &str) -> anyhow::Result<()> {
        let key = Self::object_key(tenant_id, virtual_path);
        self.store
            .put(&key, Bytes::from(body.to_owned()).into())
            .await
            .map_err(|e| anyhow::anyhow!("workspace content write failed: {e}"))?;
        Ok(())
    }

    #[instrument(skip(self), fields(tenant_id, virtual_path))]
    async fn delete(&self, tenant_id: &str, virtual_path: &str) -> anyhow::Result<()> {
        let key = Self::object_key(tenant_id, virtual_path);
        match self.store.delete(&key).await {
            Ok(()) => Ok(()),
            Err(object_store::Error::NotFound { .. }) => {
                warn!(
                    tenant_id,
                    virtual_path, "delete: object not found, skipping"
                );
                Ok(())
            }
            Err(e) => Err(anyhow::anyhow!("workspace content delete failed: {e}")),
        }
    }
}
