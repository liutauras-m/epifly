//! In-process realtime event service.
//!
//! Replaces the old Postgres LISTEN/NOTIFY backend with tokio broadcast channels.
//! Workspace changes are published by the store layer; this service fans them out
//! to per-tenant WebSocket subscriptions.

pub mod invalidation;
pub use invalidation::{InvalidationBus, InvalidationEvent, new_invalidation_bus};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};

/// Billing-related SSE event pushed to frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BillingEvent {
    QuotaWarning { tenant_id: String },
    SubscriptionUpdated { tenant_id: String },
}

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
    billing_tx: broadcast::Sender<BillingEvent>,
}

impl RealtimeService {
    pub fn new() -> Arc<Self> {
        let (billing_tx, _) = broadcast::channel(256);
        Arc::new(Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
            billing_tx,
        })
    }

    /// Subscribe to billing events (quota warnings, subscription changes).
    pub fn subscribe_billing(&self) -> broadcast::Receiver<BillingEvent> {
        self.billing_tx.subscribe()
    }

    /// Push a quota-exceeded warning to a tenant's SSE stream.
    pub async fn broadcast_quota_warning(&self, tenant_id: &str) {
        let _ = self.billing_tx.send(BillingEvent::QuotaWarning {
            tenant_id: tenant_id.to_string(),
        });
    }

    /// Push a subscription-updated event to a tenant's SSE stream.
    pub async fn broadcast_subscription_updated(&self, tenant_id: &str) {
        let _ = self.billing_tx.send(BillingEvent::SubscriptionUpdated {
            tenant_id: tenant_id.to_string(),
        });
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
}

impl Default for RealtimeService {
    fn default() -> Self {
        let (billing_tx, _) = broadcast::channel(256);
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
            billing_tx,
        }
    }
}
