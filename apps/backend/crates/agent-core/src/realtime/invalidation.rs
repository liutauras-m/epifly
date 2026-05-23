//! Per-tenant workspace invalidation bus (PR 3.A).
//!
//! The bus is a thin wrapper around `tokio::sync::broadcast`. Any part of the
//! system that mutates workspace content (ArtifactBridge, admin routes, jobs)
//! broadcasts an `InvalidationEvent`; downstream consumers (SSE streaming,
//! WebSocket push) subscribe and forward to clients.
//!
//! The SSE agent streaming path also emits `resource_invalidated` deltas
//! *inline* (before `[DONE]`) so clients don't need a second HTTP connection
//! for the common single-turn case.

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

/// The capacity of the invalidation bus ring-buffer.
/// 256 is sufficient for typical burst; lagging receivers are dropped gracefully.
pub const INVALIDATION_BUS_CAPACITY: usize = 256;

/// A single workspace-invalidation event.
///
/// `resource` identifies what kind of data changed (e.g. `"workspace"`,
/// `"threads"`, `"artifacts"`).  `scope` is the tenant ID.
/// `changed_keys` is an optional list of affected virtual paths or IDs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvalidationEvent {
    /// High-level resource type. Matches the label used by `createLiveResource`.
    pub resource: String,
    /// Tenant ID — consumers must filter to their own scope.
    pub scope: String,
    /// Optional list of affected virtual paths / IDs.  Empty means "re-fetch everything".
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_keys: Vec<String>,
}

impl InvalidationEvent {
    pub fn new(resource: impl Into<String>, scope: impl Into<String>) -> Self {
        Self {
            resource: resource.into(),
            scope: scope.into(),
            changed_keys: vec![],
        }
    }

    pub fn with_keys(mut self, keys: Vec<String>) -> Self {
        self.changed_keys = keys;
        self
    }
}

/// Shared invalidation bus — a broadcast sender that all parts of the system
/// write to.  Clone the `InvalidationBus` to get an independent sender handle.
pub type InvalidationBus = broadcast::Sender<InvalidationEvent>;

/// Create a new `InvalidationBus` + discard the initial receiver.
/// All interested consumers must call `bus.subscribe()` themselves.
pub fn new_invalidation_bus() -> InvalidationBus {
    broadcast::channel::<InvalidationEvent>(INVALIDATION_BUS_CAPACITY).0
}
