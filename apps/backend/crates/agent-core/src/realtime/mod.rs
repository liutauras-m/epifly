//! In-process realtime event service.
//!
//! Replaces the old Postgres LISTEN/NOTIFY backend with tokio broadcast channels.
//! Workspace changes are published by the store layer; this service fans them out
//! to per-tenant WebSocket subscriptions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};

/// A single workspace change event published on the broadcast channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceChangeEvent {
    pub op: String,
    pub tenant_id: String,
    pub node_id: String,
    pub kind: String,
}

type TenantSender = broadcast::Sender<WorkspaceChangeEvent>;

pub struct RealtimeService {
    channels: Arc<RwLock<HashMap<String, TenantSender>>>,
    spec_reload_tx: Arc<RwLock<Option<tokio::sync::mpsc::UnboundedSender<(String, String)>>>>,
}

impl RealtimeService {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
            spec_reload_tx: Arc::new(RwLock::new(None)),
        })
    }

    /// Register a receiver for `(namespace, tool_name)` spec-change events.
    pub async fn subscribe_capability_spec_changes(
        &self,
    ) -> tokio::sync::mpsc::UnboundedReceiver<(String, String)> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        *self.spec_reload_tx.write().await = Some(tx);
        rx
    }

    /// Subscribe to workspace changes for `tenant_id`.
    pub async fn subscribe_workspace(
        &self,
        tenant_id: &str,
    ) -> broadcast::Receiver<WorkspaceChangeEvent> {
        let mut channels = self.channels.write().await;
        let tx = channels
            .entry(tenant_id.to_owned())
            .or_insert_with(|| broadcast::channel::<WorkspaceChangeEvent>(128).0);
        tx.subscribe()
    }

    /// Publish a workspace change event to all subscribers for the event's tenant.
    pub async fn publish_workspace_change(&self, event: WorkspaceChangeEvent) {
        let channels = self.channels.read().await;
        if let Some(tx) = channels.get(&event.tenant_id) {
            let _ = tx.send(event);
        }
    }

    /// Publish a capability spec change.
    pub async fn publish_spec_change(&self, namespace: String, tool_name: String) {
        let tx_guard = self.spec_reload_tx.read().await;
        if let Some(tx) = tx_guard.as_ref() {
            let _ = tx.send((namespace, tool_name));
        }
    }
}

impl Default for RealtimeService {
    fn default() -> Self {
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
            spec_reload_tx: Arc::new(RwLock::new(None)),
        }
    }
}
