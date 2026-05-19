//! Per-tenant IAM provisioning and credential management.
//!
//! Each tenant gets a service account (access key + secret key) with an inline
//! policy restricting S3 access to `tenants/{tenant_id}/*` in the workspace
//! bucket. Credentials are returned to the caller (encrypted at rest by the
//! gateway's credential store).

use crate::RustFsAdminClient;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::instrument;

/// Raw per-tenant S3 credentials (access_key + secret_key pair).
/// The gateway encrypts these with AES-256-GCM before writing to redb.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IamCreds {
    pub access_key: String,
    pub secret_key: String,
    /// The key that should be revoked when these creds are rotated.
    pub prev_access_key: Option<String>,
}

/// Create a RustFS service account scoped to `tenants/{tenant_id}/*`.
///
/// Returns the raw credentials. The caller is responsible for encrypting and
/// storing them. Idempotent: if the service account already exists it will
/// create a new one (rotation-safe).
#[instrument(skip(client), fields(tenant_id))]
pub async fn provision_tenant(
    client: &RustFsAdminClient,
    tenant_id: &str,
) -> Result<IamCreds> {
    let (access_key, secret_key) = client
        .create_service_account("", tenant_id, &client.bucket.clone())
        .await?;

    tracing::info!(tenant_id, access_key, "provisioned per-tenant IAM service account");

    Ok(IamCreds {
        access_key,
        secret_key,
        prev_access_key: None,
    })
}

/// Delete the service account (called when a tenant is removed or keys are
/// rotated and the old key is past its grace period).
#[instrument(skip(client), fields(access_key))]
pub async fn deprovision_tenant(
    client: &RustFsAdminClient,
    access_key: &str,
) -> Result<()> {
    client.delete_service_account(access_key).await?;
    tracing::info!(access_key, "deleted tenant IAM service account");
    Ok(())
}
