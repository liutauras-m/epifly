//! Real S3 presigned URL generation using per-tenant credentials.
//!
//! Uses `object_store`'s `Signer` trait which performs SigV4 signing using the
//! tenant's IAM credentials. URLs are capped at `RUSTFS_PRESIGN_TTL_SECS`
//! (default 900s, max 3600s).

use crate::store::creds::StorageCreds;
use anyhow::{Context, Result, bail};
use object_store::{
    ObjectStore,
    aws::AmazonS3Builder,
    path::Path as OsPath,
    signer::Signer,
};
use reqwest::Method;
use std::time::Duration;
use tracing::instrument;
use url::Url;

fn presign_ttl() -> Duration {
    let secs: u64 = std::env::var("RUSTFS_PRESIGN_TTL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(900)
        .min(3600);
    Duration::from_secs(secs)
}

fn build_store(creds: &StorageCreds, endpoint: &str, bucket: &str) -> Result<impl Signer + ObjectStore> {
    AmazonS3Builder::new()
        .with_endpoint(endpoint)
        .with_bucket_name(bucket)
        .with_access_key_id(&creds.access_key)
        .with_secret_access_key(&creds.secret_key)
        .with_allow_http(true)
        .with_region("us-east-1")
        .build()
        .context("build per-tenant S3 store for presign")
}

fn validate_virtual_path(virtual_path: &str) -> Result<()> {
    if virtual_path.len() > 1024 {
        bail!("virtual_path exceeds 1024 bytes");
    }
    if virtual_path.contains("..") {
        bail!("virtual_path must not contain '..'");
    }
    if virtual_path.starts_with('/') {
        bail!("virtual_path must be relative");
    }
    for byte in virtual_path.bytes() {
        if byte < 0x20 || byte == b'\0' || byte == b'\r' || byte == b'\n' {
            bail!("virtual_path contains invalid byte 0x{byte:02x}");
        }
    }
    if virtual_path.ends_with(|c: char| c.is_whitespace()) || virtual_path.ends_with('/') {
        bail!("virtual_path must not have trailing whitespace or '/'");
    }
    Ok(())
}

/// Generate a presigned GET URL for a tenant's object.
#[instrument(skip(creds), fields(tenant_id, virtual_path))]
pub async fn presign_get(
    tenant_id: &str,
    virtual_path: &str,
    creds: &StorageCreds,
    endpoint: &str,
    bucket: &str,
    ttl: Option<Duration>,
) -> Result<Url> {
    validate_virtual_path(virtual_path)?;

    let key = OsPath::from(format!("tenants/{tenant_id}/workspaces/{virtual_path}"));
    let store = build_store(creds, endpoint, bucket)?;
    let ttl = ttl.unwrap_or_else(presign_ttl);
    let url = store
        .signed_url(Method::GET, &key, ttl)
        .await
        .context("presign GET")?;
    Ok(url)
}

/// Generate a presigned PUT URL for a direct browser → RustFS upload.
#[instrument(skip(creds), fields(tenant_id, virtual_path))]
pub async fn presign_put(
    tenant_id: &str,
    virtual_path: &str,
    creds: &StorageCreds,
    endpoint: &str,
    bucket: &str,
    ttl: Option<Duration>,
) -> Result<Url> {
    validate_virtual_path(virtual_path)?;

    let key = OsPath::from(format!("tenants/{tenant_id}/workspaces/{virtual_path}"));
    let store = build_store(creds, endpoint, bucket)?;
    let ttl = ttl.unwrap_or_else(presign_ttl);
    let url = store
        .signed_url(Method::PUT, &key, ttl)
        .await
        .context("presign PUT")?;
    Ok(url)
}

/// Generate a presigned PUT URL for the `uploads/tmp/` staging area.
#[instrument(skip(creds), fields(tenant_id, upload_id))]
pub async fn presign_tmp_put(
    tenant_id: &str,
    upload_id: &str,
    filename: &str,
    creds: &StorageCreds,
    endpoint: &str,
    bucket: &str,
) -> Result<Url> {
    let safe_filename = filename
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
        .collect::<String>();

    let key = OsPath::from(format!(
        "tenants/{tenant_id}/uploads/tmp/{upload_id}/{safe_filename}"
    ));
    let store = build_store(creds, endpoint, bucket)?;
    let url = store
        .signed_url(Method::PUT, &key, presign_ttl())
        .await
        .context("presign tmp PUT")?;
    Ok(url)
}
