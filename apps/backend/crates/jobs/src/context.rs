//! `JobContext` — shared dependencies injected into every job run.

use common::audit::AuditStore;
use std::sync::Arc;

/// Shared context provided to every scheduled and background job.
///
/// Cloned cheaply (all fields are `Arc` or `Clone`).
#[derive(Clone)]
pub struct JobContext {
    /// Append-only audit log (from `common`).
    pub audit_store: Arc<dyn AuditStore>,
    /// Qdrant base URL — passed as a string so `jobs` doesn't depend on qdrant-client directly.
    pub qdrant_url: String,
    /// MinIO / S3 endpoint (may be `None` if not configured).
    pub minio_endpoint: Option<String>,
    /// The name of the MinIO bucket.
    pub bucket: Option<String>,
}

impl JobContext {
    pub fn new(
        audit_store: Arc<dyn AuditStore>,
        qdrant_url: impl Into<String>,
        minio_endpoint: Option<String>,
        bucket: Option<String>,
    ) -> Self {
        Self {
            audit_store,
            qdrant_url: qdrant_url.into(),
            minio_endpoint,
            bucket,
        }
    }
}
