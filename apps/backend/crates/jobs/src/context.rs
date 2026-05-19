//! `JobContext` — shared dependencies injected into every job run.

use agent_core::store::CredentialStore;
use billing_core::provider::BillingProvider;
use common::audit::AuditStore;
use rustfs_admin::RustFsAdminClient;
use std::sync::Arc;

/// Shared context provided to every scheduled and background job.
///
/// Cloned cheaply (all fields are `Arc` or `Clone`).
#[derive(Clone)]
pub struct JobContext {
    /// Append-only audit log (from `common`).
    pub audit_store: Arc<dyn AuditStore>,
    /// S3/RustFS endpoint for audio/video file retrieval (may be `None` if not configured).
    pub s3_endpoint: Option<String>,
    /// The name of the S3/RustFS bucket.
    pub bucket: Option<String>,
    /// Billing provider for reconciliation jobs. `None` when Lago is not configured.
    pub billing: Option<Arc<dyn BillingProvider>>,
    /// RustFS admin client for IAM operations (key rotation, provisioning).
    pub rustfs_admin: Option<Arc<RustFsAdminClient>>,
    /// Per-tenant credential store (read/write encrypted creds in redb).
    pub cred_store: Option<Arc<CredentialStore>>,
}

impl JobContext {
    pub fn new(
        audit_store: Arc<dyn AuditStore>,
        s3_endpoint: Option<String>,
        bucket: Option<String>,
    ) -> Self {
        Self {
            audit_store,
            s3_endpoint,
            bucket,
            billing: None,
            rustfs_admin: None,
            cred_store: None,
        }
    }

    pub fn with_billing(mut self, billing: Arc<dyn BillingProvider>) -> Self {
        self.billing = Some(billing);
        self
    }

    pub fn with_rustfs(
        mut self,
        admin: Arc<RustFsAdminClient>,
        cred_store: Arc<CredentialStore>,
    ) -> Self {
        self.rustfs_admin = Some(admin);
        self.cred_store = Some(cred_store);
        self
    }
}
