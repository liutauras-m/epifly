//! Per-tenant IAM provisioning and credential management.
//!
//! In Phase 2 each tenant gets a **dedicated bucket** (`ws-{tenant_id}`) plus a
//! service account whose inline policy is scoped to that specific bucket — no
//! `s3:prefix` condition required. The bucket name is returned in `IamCreds` and
//! stored alongside the access/secret keys in `CredentialStore`.
//!
//! `TenantStorageFactory::for_tenant` reads `creds.bucket`:
//!  - `None`       → Phase 1 legacy layout (`tenants/{id}/workspaces/…` in shared bucket)
//!  - `Some(name)` → Phase 2 modern layout (`workspaces/…` in `ws-{id}` bucket)

use crate::{RustFsAdminClient, bucket::sanitize_bucket_name};
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
    /// Per-tenant dedicated bucket name (`ws-{id}`). `None` for legacy shared-bucket tenants.
    pub bucket: Option<String>,
}

/// Create a RustFS service account and (Phase 2) a dedicated per-tenant bucket.
///
/// The service account policy is scoped to the per-tenant bucket when one is
/// created, or to `tenants/{tenant_id}/*` in the shared bucket otherwise.
///
/// Returns `IamCreds` with `bucket = Some(name)` for Phase 2 tenants.
/// Idempotent: safe to re-invoke; creates a new service account on each call
/// (rotation-safe — old key must be deprovisioned separately).
#[instrument(skip(client), fields(tenant_id))]
pub async fn provision_tenant(client: &RustFsAdminClient, tenant_id: &str) -> Result<IamCreds> {
    // Compute per-tenant bucket name.
    let bucket_name = sanitize_bucket_name(&format!("ws-{tenant_id}"));

    // Ensure the bucket exists with versioning + SSE.
    client.ensure_bucket_named(&bucket_name).await?;

    // Create a service account with bucket-scoped policy (no prefix condition).
    let (access_key, secret_key) = client
        .create_bucket_scoped_service_account(tenant_id, &bucket_name)
        .await?;

    tracing::info!(
        tenant_id,
        access_key,
        bucket = bucket_name,
        "provisioned per-tenant IAM service account with dedicated bucket"
    );

    Ok(IamCreds {
        access_key,
        secret_key,
        prev_access_key: None,
        bucket: Some(bucket_name),
    })
}

/// Rotate credentials for an existing tenant: create a new service account scoped
/// to `bucket_name` (the tenant's existing bucket) without creating a new bucket.
///
/// Use this for scheduled key rotation. Use `provision_tenant` only for new tenants.
#[instrument(skip(client), fields(tenant_id))]
pub async fn rotate_tenant_credentials(
    client: &RustFsAdminClient,
    tenant_id: &str,
    bucket_name: &str,
) -> Result<IamCreds> {
    let (access_key, secret_key) = client
        .create_bucket_scoped_service_account(tenant_id, bucket_name)
        .await?;

    tracing::info!(
        tenant_id,
        access_key,
        bucket = bucket_name,
        "rotated per-tenant IAM credentials"
    );

    Ok(IamCreds {
        access_key,
        secret_key,
        prev_access_key: None,
        bucket: Some(bucket_name.to_owned()),
    })
}

/// Delete the service account (called when a tenant is removed or keys are
/// rotated and the old key is past its grace period).
#[instrument(skip(client), fields(access_key))]
pub async fn deprovision_tenant(client: &RustFsAdminClient, access_key: &str) -> Result<()> {
    client.delete_service_account(access_key).await?;
    tracing::info!(access_key, "deleted tenant IAM service account");
    Ok(())
}
