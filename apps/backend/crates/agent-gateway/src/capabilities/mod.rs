//! Capability subsystem — built-in capability providers and dispatch context.
//!
//! # `CapabilityExecutionContext`
//! The gateway constructs a `CapabilityExecutionContext` once per tool invocation
//! and passes it to each capability. Capabilities receive only the **narrow
//! `WorkspaceStorage` trait** — they cannot call multipart, staging, presign, or
//! raw-credential APIs even by mistake.
//!
//! Phase 3 acceptance:
//! - No capability implementation imports `object_store`, `TenantStorage`, or `S3_BUCKET`.
//! - `CapabilityExecutionContext::workspace` is the single documented path to storage.
//! - The `lint-capability-storage` CI guard enforces this at commit time.

pub mod transcribe_video;
pub mod workspace;

use agent_core::{WorkspaceStorage, StorageQuotaService};
use common::audit::AuditStore;
use std::sync::{Arc, atomic::AtomicBool};

/// Per-invocation context injected into every capability tool call.
///
/// The gateway constructs this once per dispatch, wrapping the tenant's
/// `TenantStorage` as `Arc<dyn WorkspaceStorage>`. Capabilities cannot call
/// `finalize_staged_upload`, `presign_staging_put`, see `StorageCreds`, or
/// instantiate an `ObjectStore` directly — the trait simply does not expose them.
///
/// **Ownership:** lives in `agent-gateway` until a separate agent-runtime crate
/// is introduced, at which point both this struct and `WorkspaceStorage` move there.
pub struct CapabilityExecutionContext {
    /// The tenant this invocation belongs to.
    pub tenant_id: String,
    /// Identifier for the user or agent actor that triggered the tool call.
    pub actor: String,
    /// Narrow, auditable workspace storage surface. No multipart, no staging, no creds.
    pub workspace: Arc<dyn WorkspaceStorage>,
    /// Per-tenant quota state — checked by storage ops before writing.
    pub quota: Arc<StorageQuotaService>,
    /// Audit sink — capability code may emit structured events here.
    pub audit: Arc<dyn AuditStore>,
    /// Cancellation signal — capabilities should honour this for long-running work.
    /// Set to `true` to request cancellation.
    pub cancel: Arc<AtomicBool>,
}

impl CapabilityExecutionContext {
    pub fn new(
        tenant_id: impl Into<String>,
        actor: impl Into<String>,
        workspace: Arc<dyn WorkspaceStorage>,
        quota: Arc<StorageQuotaService>,
        audit: Arc<dyn AuditStore>,
        cancel: Arc<AtomicBool>,
    ) -> Self {
        Self {
            tenant_id: tenant_id.into(),
            actor: actor.into(),
            workspace,
            quota,
            audit,
            cancel,
        }
    }
}
