//! RustFsContentStore — per-tenant S3 object store for workspace markdown bodies.
//!
//! Every workspace IO goes through a per-tenant `AmazonS3` client built from
//! credentials stored in `CredentialStore`. Falls back to root credentials when
//! `RUSTFS_PER_TENANT_IAM=off` (dev only).
//!
//! SSE-S3 is enforced at the bucket level via declarative bootstrap (PUT bucket
//! encryption). No per-request SSE headers are needed.
//!
//! Key scheme: `tenants/{tenant_id}/workspaces/{virtual_path}`

use crate::store::creds::{CredentialStore, StorageCreds};
use async_trait::async_trait;
use bytes::Bytes;
use common::memory::store::WorkspaceContentStore;
use moka::future::Cache;
use object_store::{
    ObjectStore, PutOptions, PutPayload,
    aws::AmazonS3Builder,
    path::Path as OsPath,
};
use std::sync::Arc;
use tracing::{instrument, warn};

fn s3_endpoint() -> String {
    std::env::var("S3_ENDPOINT").unwrap_or_else(|_| "http://rustfs:9000".into())
}

fn s3_bucket() -> String {
    std::env::var("S3_BUCKET").unwrap_or_else(|_| "workspace".into())
}

fn per_tenant_iam() -> bool {
    std::env::var("RUSTFS_PER_TENANT_IAM").as_deref() != Ok("off")
}

/// Build an S3 store from explicit credentials.
fn build_store_with_creds(creds: &StorageCreds) -> anyhow::Result<Arc<dyn ObjectStore>> {
    let store = AmazonS3Builder::new()
        .with_endpoint(s3_endpoint())
        .with_bucket_name(s3_bucket())
        .with_access_key_id(&creds.access_key)
        .with_secret_access_key(&creds.secret_key)
        .with_allow_http(true)
        .with_region("us-east-1")
        .build()?;
    Ok(Arc::new(store))
}

/// Build an S3 store from the root admin credentials (used when per-tenant IAM
/// is disabled or during initial tenant provisioning).
pub fn build_root_store() -> anyhow::Result<Arc<dyn ObjectStore>> {
    let access_key = std::env::var("RUSTFS_ROOT_ACCESS_KEY")
        .or_else(|_| std::env::var("AWS_ACCESS_KEY_ID"))
        .unwrap_or_else(|_| "rustfsadmin".into());
    let secret_key = std::env::var("RUSTFS_ROOT_SECRET_KEY")
        .or_else(|_| std::env::var("AWS_SECRET_ACCESS_KEY"))
        .unwrap_or_else(|_| "rustfsadmin".into());
    build_store_with_creds(&StorageCreds { access_key, secret_key, created_at: 0 })
}

pub struct RustFsContentStore {
    /// LRU cache of per-tenant S3 clients (keyed by tenant_id).
    /// Max 1024 entries, 5 min TTL.
    client_cache: Cache<String, Arc<dyn ObjectStore>>,
    /// Credential store — decrypts and caches per-tenant IAM credentials.
    cred_store: Option<Arc<CredentialStore>>,
}

impl RustFsContentStore {
    pub fn new(cred_store: Option<Arc<CredentialStore>>) -> Arc<Self> {
        Arc::new(Self {
            client_cache: Cache::builder()
                .max_capacity(1024)
                .time_to_live(std::time::Duration::from_secs(300))
                .build(),
            cred_store,
        })
    }

    /// Resolve the S3 client for a given tenant. Falls back to root credentials
    /// when per-tenant IAM is off or when no creds are provisioned yet.
    async fn client_for(&self, tenant_id: &str) -> Arc<dyn ObjectStore> {
        if let Some(cached) = self.client_cache.get(tenant_id).await {
            return cached;
        }

        let store = if per_tenant_iam() {
            if let Some(ref cs) = self.cred_store {
                match cs.load(tenant_id).await {
                    Ok(Some(creds)) => match build_store_with_creds(&creds) {
                        Ok(s) => s,
                        Err(e) => {
                            warn!(tenant_id, error = %e, "per-tenant store build failed; falling back to root");
                            build_root_store().unwrap_or_else(|_| panic!("root store"))
                        }
                    },
                    Ok(None) => {
                        warn!(tenant_id, "no IAM creds provisioned yet; falling back to root");
                        build_root_store().unwrap_or_else(|_| panic!("root store"))
                    }
                    Err(e) => {
                        warn!(tenant_id, error = %e, "cred load failed; falling back to root");
                        build_root_store().unwrap_or_else(|_| panic!("root store"))
                    }
                }
            } else {
                build_root_store().unwrap_or_else(|_| panic!("root store"))
            }
        } else {
            build_root_store().unwrap_or_else(|_| panic!("root store"))
        };

        self.client_cache.insert(tenant_id.to_string(), Arc::clone(&store)).await;
        store
    }

    pub fn object_key(tenant_id: &str, virtual_path: &str) -> OsPath {
        OsPath::from(format!("tenants/{tenant_id}/workspaces/{virtual_path}"))
    }
}

#[async_trait]
impl WorkspaceContentStore for RustFsContentStore {
    #[instrument(skip(self), fields(tenant_id, virtual_path))]
    async fn read(&self, tenant_id: &str, virtual_path: &str) -> anyhow::Result<String> {
        let store = self.client_for(tenant_id).await;
        let key = Self::object_key(tenant_id, virtual_path);
        match store.get(&key).await {
            Ok(result) => {
                let bytes = result.bytes().await?;
                Ok(String::from_utf8_lossy(&bytes).into_owned())
            }
            Err(object_store::Error::NotFound { .. }) => Ok(String::new()),
            Err(e) => Err(anyhow::anyhow!("content read failed: {e}")),
        }
    }

    #[instrument(skip(self, body), fields(tenant_id, virtual_path))]
    async fn write(&self, tenant_id: &str, virtual_path: &str, body: &str) -> anyhow::Result<()> {
        let store = self.client_for(tenant_id).await;
        let key = Self::object_key(tenant_id, virtual_path);

        let mut opts = PutOptions::default();
        opts.attributes
            .insert(object_store::Attribute::ContentType, "text/markdown".into());

        let payload: PutPayload = Bytes::from(body.to_owned()).into();
        store
            .put_opts(&key, payload, opts)
            .await
            .map_err(|e| anyhow::anyhow!("content write failed: {e}"))?;
        Ok(())
    }

    #[instrument(skip(self), fields(tenant_id, virtual_path))]
    async fn delete(&self, tenant_id: &str, virtual_path: &str) -> anyhow::Result<()> {
        let store = self.client_for(tenant_id).await;
        let key = Self::object_key(tenant_id, virtual_path);
        match store.delete(&key).await {
            Ok(()) => Ok(()),
            Err(object_store::Error::NotFound { .. }) => {
                warn!(tenant_id, virtual_path, "delete: object not found, skipping");
                Ok(())
            }
            Err(e) => Err(anyhow::anyhow!("content delete failed: {e}")),
        }
    }
}
