//! `JobContext` — shared dependencies injected into every job run.

use crate::jobs::thread_projection::ProjectionCoalescer;
use agent_core::{
    indexing::embedding_service::EmbeddingService,
    store::{
        CredentialStore, QdrantVectorStore, ThreadProjectionStore,
        tenant_storage::TenantStorageFactory,
    },
};
use billing_core::provider::BillingProvider;
use common::audit::AuditStore;
use common::memory::store::{ThreadStore, WorkspaceContentStore, WorkspaceStore};
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
    /// Tenant storage factory — used by the bucket migration backfill job.
    pub tenant_storage_factory: Option<Arc<TenantStorageFactory>>,
    /// Workspace metadata store — used by the bucket migration job.
    pub workspace_store: Option<Arc<dyn WorkspaceStore>>,
    /// Workspace content store (RustFS markdown bodies) — used by `WorkspaceIndexJob`.
    pub workspace_content: Option<Arc<dyn WorkspaceContentStore>>,
    /// Embedding service — used by `WorkspaceIndexJob`.
    pub embedding_service: Option<Arc<dyn EmbeddingService>>,
    /// Vector store (Qdrant) — used by `WorkspaceIndexJob`.
    pub vector_store: Option<Arc<QdrantVectorStore>>,
    /// Thread store — used by `ThreadProjectionJob`.
    pub thread_store: Option<Arc<dyn ThreadStore>>,
    /// Thread projection durable index — used by `ThreadProjectionJob`.
    pub thread_projection_store: Option<Arc<dyn ThreadProjectionStore>>,
    /// Coalescing guard for thread projection jobs (at-most-one-running per thread).
    pub projection_coalescer: Option<Arc<ProjectionCoalescer>>,
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
            tenant_storage_factory: None,
            workspace_store: None,
            workspace_content: None,
            embedding_service: None,
            vector_store: None,
            thread_store: None,
            thread_projection_store: None,
            projection_coalescer: None,
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

    pub fn with_storage(
        mut self,
        factory: Arc<TenantStorageFactory>,
        workspace_store: Arc<dyn WorkspaceStore>,
    ) -> Self {
        self.tenant_storage_factory = Some(factory);
        self.workspace_store = Some(workspace_store);
        self
    }

    pub fn with_indexing(
        mut self,
        workspace_content: Arc<dyn WorkspaceContentStore>,
        embedding_service: Arc<dyn EmbeddingService>,
        vector_store: Arc<QdrantVectorStore>,
    ) -> Self {
        self.workspace_content = Some(workspace_content);
        self.embedding_service = Some(embedding_service);
        self.vector_store = Some(vector_store);
        self
    }

    pub fn with_thread_projection(
        mut self,
        thread_store: Arc<dyn ThreadStore>,
        projection_store: Arc<dyn ThreadProjectionStore>,
        coalescer: Arc<ProjectionCoalescer>,
    ) -> Self {
        self.thread_store = Some(thread_store);
        self.thread_projection_store = Some(projection_store);
        self.projection_coalescer = Some(coalescer);
        self
    }
}
