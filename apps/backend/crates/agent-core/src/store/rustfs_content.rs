//! RustFsContentStore — reads/writes markdown bodies for Conversation nodes.
//!
//! Wraps `object_store::ObjectStore` (S3-compatible), pointed at RustFS
//! via the standard `AWS_*` / `S3_*` env vars.  RustFS exposes the same
//! S3 API as MinIO, so the logic is identical — only the name changes.
//!
//! Object keys: `tenants/{tenant_id}/workspaces/{virtual_path}`

use async_trait::async_trait;
use bytes::Bytes;
use common::memory::store::WorkspaceContentStore;
use object_store::{ObjectStore, aws::AmazonS3Builder, path::Path as OsPath};
use std::sync::Arc;
use tracing::{instrument, warn};

pub struct RustFsContentStore {
    store: Arc<dyn ObjectStore>,
}

impl RustFsContentStore {
    pub fn new(store: Arc<dyn ObjectStore>) -> Self {
        Self { store }
    }

    /// Build from environment variables:
    /// `S3_ENDPOINT`, `S3_BUCKET`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`.
    pub fn from_env() -> anyhow::Result<Arc<Self>> {
        let endpoint = std::env::var("S3_ENDPOINT")
            .unwrap_or_else(|_| "http://rustfs:9000".into());
        let bucket = std::env::var("S3_BUCKET")
            .unwrap_or_else(|_| "workspace".into());
        let access_key = std::env::var("AWS_ACCESS_KEY_ID")
            .unwrap_or_else(|_| "minioadmin".into());
        let secret_key = std::env::var("AWS_SECRET_ACCESS_KEY")
            .unwrap_or_else(|_| "minioadmin".into());

        let store = AmazonS3Builder::new()
            .with_endpoint(&endpoint)
            .with_bucket_name(&bucket)
            .with_access_key_id(&access_key)
            .with_secret_access_key(&secret_key)
            .with_allow_http(true)
            .with_region("us-east-1")
            .build()?;

        Ok(Arc::new(Self { store: Arc::new(store) }))
    }

    pub fn object_key(tenant_id: &str, virtual_path: &str) -> OsPath {
        OsPath::from(format!("tenants/{tenant_id}/workspaces/{virtual_path}"))
    }

    pub fn inner(&self) -> Arc<dyn ObjectStore> {
        Arc::clone(&self.store)
    }
}

#[async_trait]
impl WorkspaceContentStore for RustFsContentStore {
    #[instrument(skip(self))]
    async fn read(&self, tenant_id: &str, virtual_path: &str) -> anyhow::Result<String> {
        let key = Self::object_key(tenant_id, virtual_path);
        match self.store.get(&key).await {
            Ok(result) => {
                let bytes = result.bytes().await?;
                Ok(String::from_utf8_lossy(&bytes).into_owned())
            }
            Err(object_store::Error::NotFound { .. }) => Ok(String::new()),
            Err(e) => Err(anyhow::anyhow!("content read failed: {e}")),
        }
    }

    #[instrument(skip(self, body))]
    async fn write(&self, tenant_id: &str, virtual_path: &str, body: &str) -> anyhow::Result<()> {
        let key = Self::object_key(tenant_id, virtual_path);
        self.store
            .put(&key, Bytes::from(body.to_owned()).into())
            .await
            .map_err(|e| anyhow::anyhow!("content write failed: {e}"))?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn delete(&self, tenant_id: &str, virtual_path: &str) -> anyhow::Result<()> {
        let key = Self::object_key(tenant_id, virtual_path);
        match self.store.delete(&key).await {
            Ok(()) => Ok(()),
            Err(object_store::Error::NotFound { .. }) => {
                warn!(tenant_id, virtual_path, "delete: object not found, skipping");
                Ok(())
            }
            Err(e) => Err(anyhow::anyhow!("content delete failed: {e}")),
        }
    }
}
