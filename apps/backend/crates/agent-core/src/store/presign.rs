//! Presigned URL helpers — thin wrappers that delegate entirely to `TenantStorage`.
//!
//! All S3 key construction happens inside `TenantStorage`; no path literals here.
//! New code should call `TenantStorageFactory::for_tenant` and use its methods directly.

use crate::store::creds::StorageCreds;
use crate::store::tenant_storage::{StorageError, TenantStorage, VirtualPath};
use std::time::Duration;
use url::Url;

fn presign_ttl_default() -> Duration {
    let secs: u64 = std::env::var("RUSTFS_PRESIGN_TTL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(900)
        .min(3600);
    Duration::from_secs(secs)
}

/// Generate a presigned GET URL for a tenant workspace object.
pub async fn presign_get(
    tenant_id: &str,
    virtual_path: &str,
    creds: &StorageCreds,
    endpoint: &str,
    bucket: &str,
    ttl: Option<Duration>,
) -> Result<Url, StorageError> {
    let storage = TenantStorage::from_raw_creds(tenant_id, creds.clone(), endpoint, bucket)?;
    let vp = VirtualPath::parse(virtual_path)?;
    storage.presign_workspace_get(&vp, ttl.unwrap_or_else(presign_ttl_default), None).await
}

/// Generate a presigned PUT URL for a tenant workspace object.
pub async fn presign_put(
    tenant_id: &str,
    virtual_path: &str,
    creds: &StorageCreds,
    endpoint: &str,
    bucket: &str,
    ttl: Option<Duration>,
) -> Result<Url, StorageError> {
    let storage = TenantStorage::from_raw_creds(tenant_id, creds.clone(), endpoint, bucket)?;
    let vp = VirtualPath::parse(virtual_path)?;
    storage.presign_workspace_put(&vp, ttl.unwrap_or_else(presign_ttl_default)).await
}

/// Generate a presigned PUT URL for the staging area.
pub async fn presign_tmp_put(
    tenant_id: &str,
    upload_id: &str,
    filename: &str,
    creds: &StorageCreds,
    endpoint: &str,
    bucket: &str,
) -> Result<Url, StorageError> {
    let storage = TenantStorage::from_raw_creds(tenant_id, creds.clone(), endpoint, bucket)?;
    storage.presign_staging_put(upload_id, filename, presign_ttl_default()).await
}
