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
    /// S3/RustFS endpoint for audio/video file retrieval (may be `None` if not configured).
    pub s3_endpoint: Option<String>,
    /// The name of the S3/RustFS bucket.
    pub bucket: Option<String>,
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
        }
    }
}
