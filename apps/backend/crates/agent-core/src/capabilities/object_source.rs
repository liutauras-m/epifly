//! Tenant-scoped object reads for capabilities that take file inputs (vision chains).
//!
//! A capability input such as `image_path` is an **object key in the calling
//! tenant's storage** — never a local filesystem path or a URL. Reads go through the
//! tenant's own (per-tenant IAM) credentials and the key must belong to the tenant.

use crate::context::tenant::TenantContext;
use crate::store::{StorageError, TenantStorageFactory};
use async_trait::async_trait;
use bytes::Bytes;

#[async_trait]
pub trait TenantObjectSource: Send + Sync + 'static {
    async fn read(&self, tenant: &TenantContext, object_key: &str) -> anyhow::Result<Bytes>;
}

/// Source for deployments/tests without object storage: every read fails.
pub struct NoObjectSource;

#[async_trait]
impl TenantObjectSource for NoObjectSource {
    async fn read(&self, _tenant: &TenantContext, _object_key: &str) -> anyhow::Result<Bytes> {
        anyhow::bail!("object storage is not configured")
    }
}

/// Reject anything that is not a plain relative object key.
pub fn validate_object_key(key: &str) -> anyhow::Result<()> {
    let bad = key.is_empty()
        || key.len() > 1024
        || key.starts_with('/')
        || key.contains("://")
        || key.contains('\\')
        || key.chars().any(char::is_control)
        || key.split('/').any(|seg| seg == ".." || seg == ".");
    if bad {
        anyhow::bail!("image_path must be an object key in your workspace storage");
    }
    Ok(())
}

#[async_trait]
impl TenantObjectSource for TenantStorageFactory {
    async fn read(&self, tenant: &TenantContext, object_key: &str) -> anyhow::Result<Bytes> {
        validate_object_key(object_key)?;
        let storage = self.for_tenant(tenant.tenant_id.as_str()).await?;
        match storage.get_owned_object(object_key).await {
            Ok(b) => Ok(b),
            Err(StorageError::NotFound) => anyhow::bail!("object not found: {object_key}"),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_key_validation() {
        assert!(validate_object_key("tenants/t1/u1/scan.png").is_ok());
        assert!(validate_object_key("attachments/u1/scan.png").is_ok());
        for bad in [
            "",
            "/data/conusai.redb",
            "../etc/passwd",
            "tenants/t1/../t2/x.png",
            "http://169.254.169.254/latest/meta-data",
            "https://example.com/x.png",
            "C:\\x.png",
        ] {
            assert!(validate_object_key(bad).is_err(), "{bad} must be rejected");
        }
    }
}
