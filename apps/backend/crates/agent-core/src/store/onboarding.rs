//! TenantOnboardingService — single SRP entry point for new-tenant provisioning.
//!
//! Idempotent: safe to re-invoke after partial failure.
//! Every non-system tenant gets a named root folder (`DEFAULT_TENANT_ROOT_NAME`)
//! at provisioning time, fixing the "No folders yet" UX bug.

use crate::store::creds::{CredentialStore, StorageCreds};
use crate::store::tenant_storage::{DEFAULT_TENANT_ROOT_NAME, TenantStorageFactory};
use common::memory::store::WorkspaceStore;
use rustfs_admin::RustFsAdminClient;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tracing::{instrument, warn};

/// Tenant kind controls whether a default workspace root folder is created.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TenantKind {
    /// Regular user-facing tenant — gets a `Workspace` root folder.
    Normal,
    /// System / observability tenants — no user-visible workspace.
    System,
}

impl TenantKind {
    pub fn as_str(self) -> &'static str {
        match self {
            TenantKind::Normal => "normal",
            TenantKind::System => "system",
        }
    }
}

/// Options for `TenantOnboardingService::provision`.
pub struct OnboardingOptions {
    pub kind: TenantKind,
    /// Override the default root folder name. `None` → `DEFAULT_TENANT_ROOT_NAME`.
    pub root_name: Option<String>,
}

impl Default for OnboardingOptions {
    fn default() -> Self {
        Self { kind: TenantKind::Normal, root_name: None }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum OnboardingError {
    #[error("IAM provisioning failed: {0}")]
    Iam(#[source] anyhow::Error),
    #[error("workspace store error: {0}")]
    Store(#[source] anyhow::Error),
    #[error("storage factory error: {0}")]
    Storage(#[from] crate::store::tenant_storage::StorageError),
    #[error("credential store error: {0}")]
    Creds(#[source] anyhow::Error),
}

pub struct TenantOnboardingService {
    workspace_store: Arc<dyn WorkspaceStore>,
    storage_factory: Arc<TenantStorageFactory>,
    creds_store: Arc<CredentialStore>,
    admin: Arc<RustFsAdminClient>,
    /// Total successful provisions (kind=normal or kind=system). Synced to Prometheus by gateway.
    pub onboarding_total: Arc<AtomicU64>,
    /// Counts `_meta/seeded` marker write failures (not fatal; DB record is authoritative).
    pub marker_failed: Arc<AtomicU64>,
}

impl TenantOnboardingService {
    pub fn new(
        workspace_store: Arc<dyn WorkspaceStore>,
        storage_factory: Arc<TenantStorageFactory>,
        creds_store: Arc<CredentialStore>,
        admin: Arc<RustFsAdminClient>,
    ) -> Arc<Self> {
        Arc::new(Self {
            workspace_store,
            storage_factory,
            creds_store,
            admin,
            onboarding_total: Arc::new(AtomicU64::new(0)),
            marker_failed: Arc::new(AtomicU64::new(0)),
        })
    }

    /// Provision a tenant: create IAM service account, then seed the default workspace.
    ///
    /// Idempotent — safe to call multiple times; subsequent calls return early if already seeded.
    #[instrument(
        name = "tenant_onboarding",
        skip(self, owner_id, opts),
        fields(tenant_id, kind = ?opts.kind)
    )]
    pub async fn provision(
        &self,
        tenant_id: &str,
        owner_id: &str,
        opts: OnboardingOptions,
    ) -> Result<(), OnboardingError> {
        // 1. Provision IAM service account (idempotent on the RustFS side).
        let iam_creds = rustfs_admin::iam::provision_tenant(&self.admin, tenant_id)
            .await
            .map_err(OnboardingError::Iam)?;

        // 2. Persist IAM credentials (bucket = Some(name) when provision_tenant created a
        //    per-tenant bucket; None for legacy shared-bucket provisioning).
        self.creds_store
            .store(
                tenant_id,
                &StorageCreds {
                    access_key: iam_creds.access_key,
                    secret_key: iam_creds.secret_key,
                    created_at: 0,
                    bucket: iam_creds.bucket,
                },
            )
            .await
            .map_err(OnboardingError::Creds)?;

        // 3. Idempotency check — skip root folder if already seeded.
        if self
            .workspace_store
            .is_tenant_seeded(tenant_id)
            .await
            .map_err(OnboardingError::Store)?
        {
            return Ok(());
        }

        // 4. System tenants skip root folder creation.
        if opts.kind == TenantKind::System {
            self.workspace_store
                .mark_tenant_seeded(tenant_id)
                .await
                .map_err(OnboardingError::Store)?;
            return Ok(());
        }

        // 5. Create the protected root workspace folder (parent_id = None).
        let name = opts.root_name.as_deref().unwrap_or(DEFAULT_TENANT_ROOT_NAME);
        self.workspace_store
            .create_protected_root_folder(tenant_id, owner_id, name)
            .await
            .map_err(OnboardingError::Store)?;

        // 6. Best-effort _meta/seeded marker in object storage (not authoritative).
        let storage = self.storage_factory.for_tenant(tenant_id).await?;
        if let Err(err) = storage.write_seeded_marker().await {
            warn!(
                tenant_id,
                error = %err,
                "seeded marker write failed; DB record is authoritative"
            );
            self.marker_failed.fetch_add(1, Ordering::Relaxed);
        }

        // 7. Flip tenant_seeded flag.
        self.workspace_store
            .mark_tenant_seeded(tenant_id)
            .await
            .map_err(OnboardingError::Store)?;

        self.onboarding_total.fetch_add(1, Ordering::Relaxed);
        tracing::info!(tenant_id, owner_id, kind = opts.kind.as_str(), "tenant provisioned");
        Ok(())
    }
}
