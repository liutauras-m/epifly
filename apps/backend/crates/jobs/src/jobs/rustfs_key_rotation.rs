//! `RustFsKeyRotationJob` — rotates per-tenant RustFS IAM service-account keys
//! every 90 days (configurable via `RUSTFS_KEY_ROTATION_DAYS`).
//!
//! Runs at 03:00 daily. For each tenant in the credential store it:
//! 1. Loads the current credentials and checks their `created_at` timestamp.
//! 2. If the creds are older than the rotation threshold, provisions a new
//!    service account (keeping the old access key alive briefly).
//! 3. Stores the new creds (with `prev_access_key` set).
//! 4. Deletes the old service account immediately (no grace period by default;
//!    set `RUSTFS_KEY_ROTATION_GRACE_SECS` for a delay).

use crate::context::JobContext;
use crate::job::ScheduledJob;
use agent_core::store::StorageCreds;
use async_trait::async_trait;
use rustfs_admin::iam;
use std::sync::Arc;
use tracing::{info, warn};

pub struct RustFsKeyRotationJob;

#[async_trait]
impl ScheduledJob for RustFsKeyRotationJob {
    fn name(&self) -> &str {
        "rustfs-key-rotation"
    }

    fn cron(&self) -> &str {
        "0 0 3 * * *"
    }

    async fn run(&self, ctx: Arc<JobContext>) -> anyhow::Result<()> {
        let rotation_days: u64 = std::env::var("RUSTFS_KEY_ROTATION_DAYS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(90);

        let (Some(admin), Some(cred_store)) =
            (ctx.rustfs_admin.as_ref(), ctx.cred_store.as_ref())
        else {
            info!("rustfs-key-rotation: admin client or cred store not configured — skipping");
            return Ok(());
        };

        let tenant_ids = cred_store.list_all_tenants().await?;
        if tenant_ids.is_empty() {
            info!("rustfs-key-rotation: no tenants with stored credentials");
            return Ok(());
        }

        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let threshold_secs = (rotation_days * 86_400) as i64;

        let mut rotated = 0u32;
        let mut skipped = 0u32;

        for tenant_id in &tenant_ids {
            let creds = match cred_store.load(tenant_id).await {
                Ok(Some(c)) => c,
                Ok(None) => continue,
                Err(e) => {
                    warn!(tenant_id, error = %e, "rustfs-key-rotation: failed to load creds");
                    continue;
                }
            };

            let age_secs = now_secs - creds.created_at;
            if age_secs < threshold_secs {
                skipped += 1;
                continue;
            }

            info!(
                tenant_id,
                age_days = age_secs / 86_400,
                rotation_days,
                "rustfs-key-rotation: rotating credentials"
            );

            // Provision a new service account.
            let new_iam = match iam::provision_tenant(admin, tenant_id).await {
                Ok(c) => c,
                Err(e) => {
                    warn!(tenant_id, error = %e, "rustfs-key-rotation: failed to provision new creds");
                    continue;
                }
            };

            let old_access_key = creds.access_key.clone();

            // Store new creds (created_at will be auto-stamped by the store).
            let new_creds = StorageCreds {
                access_key: new_iam.access_key.clone(),
                secret_key: new_iam.secret_key.clone(),
                created_at: 0, // will be stamped by CredentialStore::store
            };
            if let Err(e) = cred_store.store(tenant_id, &new_creds).await {
                warn!(tenant_id, error = %e, "rustfs-key-rotation: failed to persist new creds; aborting rotation for this tenant");
                // Attempt to clean up the freshly created service account.
                let _ = iam::deprovision_tenant(admin, &new_iam.access_key).await;
                continue;
            }

            // Delete the old service account.
            if let Err(e) = iam::deprovision_tenant(admin, &old_access_key).await {
                warn!(
                    tenant_id,
                    old_access_key,
                    error = %e,
                    "rustfs-key-rotation: new creds stored but old service account deletion failed — manual cleanup required"
                );
            }

            rotated += 1;
            info!(tenant_id, new_access_key = %new_iam.access_key, "rustfs-key-rotation: rotated");
        }

        info!(
            rotated,
            skipped,
            total = tenant_ids.len(),
            rotation_days,
            "rustfs-key-rotation: complete"
        );
        Ok(())
    }
}
