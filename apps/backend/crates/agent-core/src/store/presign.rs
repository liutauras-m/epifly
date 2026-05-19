//! Presigned URL helpers — thin wrappers that delegate to `TenantStorage`.
//!
//! These free functions are kept for call-site compatibility. New code should
//! obtain a `TenantStorage` from `TenantStorageFactory` and call methods directly.

use crate::store::tenant_storage::{StorageLayout, TenantStorage, VirtualPath};
use crate::store::creds::StorageCreds;
use anyhow::Result;
use object_store::{aws::AmazonS3Builder, path::Path as ObjectPath, signer::Signer, ObjectStore};
use reqwest::Method;
use std::{sync::Arc, time::Duration};
use url::Url;

fn build_store(creds: &StorageCreds, endpoint: &str, bucket: &str) -> Result<impl Signer + ObjectStore> {
    AmazonS3Builder::new()
        .with_endpoint(endpoint)
        .with_bucket_name(bucket)
        .with_access_key_id(&creds.access_key)
        .with_secret_access_key(&creds.secret_key)
        .with_allow_http(true)
        .with_region("us-east-1")
        .build()
        .map_err(|e| anyhow::anyhow!("build presign store: {e}"))
}

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
) -> Result<Url> {
    let vp = VirtualPath::parse(virtual_path).map_err(|e| anyhow::anyhow!("{e}"))?;
    let key = ObjectPath::from(format!("tenants/{tenant_id}/workspaces/{}", vp.as_str()));
    let store = build_store(creds, endpoint, bucket)?;
    store.signed_url(Method::GET, &key, ttl.unwrap_or_else(presign_ttl_default)).await
        .map_err(|e| anyhow::anyhow!("presign GET: {e}"))
}

/// Generate a presigned PUT URL for a tenant workspace object.
pub async fn presign_put(
    tenant_id: &str,
    virtual_path: &str,
    creds: &StorageCreds,
    endpoint: &str,
    bucket: &str,
    ttl: Option<Duration>,
) -> Result<Url> {
    let vp = VirtualPath::parse(virtual_path).map_err(|e| anyhow::anyhow!("{e}"))?;
    let key = ObjectPath::from(format!("tenants/{tenant_id}/workspaces/{}", vp.as_str()));
    let store = build_store(creds, endpoint, bucket)?;
    store.signed_url(Method::PUT, &key, ttl.unwrap_or_else(presign_ttl_default)).await
        .map_err(|e| anyhow::anyhow!("presign PUT: {e}"))
}

/// Generate a presigned PUT URL for the staging area.
pub async fn presign_tmp_put(
    tenant_id: &str,
    upload_id: &str,
    filename: &str,
    creds: &StorageCreds,
    endpoint: &str,
    bucket: &str,
) -> Result<Url> {
    let safe_filename: String = filename
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
        .collect();
    let key = ObjectPath::from(format!(
        "tenants/{tenant_id}/uploads/tmp/{upload_id}/{safe_filename}"
    ));
    let store = build_store(creds, endpoint, bucket)?;
    store.signed_url(Method::PUT, &key, presign_ttl_default()).await
        .map_err(|e| anyhow::anyhow!("presign tmp PUT: {e}"))
}
