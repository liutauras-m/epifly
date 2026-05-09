//! `JobContext` — shared dependencies injected into every job run.

use common::audit::AuditStore;
use sqlx::PgPool;
use std::sync::Arc;

/// Shared context provided to every scheduled and background job.
///
/// Cloned cheaply (all fields are `Arc` or `Clone`).
#[derive(Clone)]
pub struct JobContext {
    /// Append-only audit log (from `common`).
    pub audit_store: Arc<dyn AuditStore>,
    /// Optional Postgres connection pool. `None` in in-memory test mode.
    pub pool: Option<PgPool>,
    /// MinIO / S3 endpoint (may be `None` if not configured).
    pub minio_endpoint: Option<String>,
    /// The name of the MinIO bucket.
    pub bucket: Option<String>,
}

impl JobContext {
    pub fn new(
        audit_store: Arc<dyn AuditStore>,
        pool: Option<PgPool>,
        minio_endpoint: Option<String>,
        bucket: Option<String>,
    ) -> Self {
        Self {
            audit_store,
            pool,
            minio_endpoint,
            bucket,
        }
    }
}
