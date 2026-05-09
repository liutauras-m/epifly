//! Postgres `LISTEN/NOTIFY`-based realtime event service.
//!
//! Uses `sqlx::postgres::PgListener` to subscribe to the `workspace_changes`
//! channel and fan-out events to registered broadcast receivers.  No extra
//! infrastructure needed — pure Postgres.

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use tracing::{error, info, warn};

/// A single workspace change event published on the broadcast channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceChangeEvent {
    pub op: String,
    pub tenant_id: String,
    pub node_id: String,
    pub kind: String,
}

type TenantSender = broadcast::Sender<WorkspaceChangeEvent>;

/// Broadcasts Postgres `workspace_changes` notifications to WebSocket clients.
///
/// One `PgListener` loop is started per `RealtimeService` instance; it fans out
/// to per-tenant broadcast channels so each WS client only receives events for
/// its own tenant.
type SpecReloadTx = Arc<RwLock<Option<tokio::sync::mpsc::UnboundedSender<(String, String)>>>>;

pub struct RealtimeService {
    pool: PgPool,
    /// per-tenant broadcast senders; created lazily on first subscription.
    channels: Arc<RwLock<HashMap<String, TenantSender>>>,
    /// optional sender for capability_specs_changed events (namespace, tool_name).
    spec_reload_tx: SpecReloadTx,
}

impl RealtimeService {
    pub fn new(pool: PgPool) -> Arc<Self> {
        let svc = Arc::new(Self {
            pool,
            channels: Arc::new(RwLock::new(HashMap::new())),
            spec_reload_tx: Arc::new(RwLock::new(None)),
        });
        svc.clone().spawn_listener();
        svc
    }

    /// Register a channel that receives `(namespace, tool_name)` tuples whenever
    /// a `capability_specs_changed` notification arrives.
    pub async fn subscribe_capability_spec_changes(
        &self,
    ) -> tokio::sync::mpsc::UnboundedReceiver<(String, String)> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        *self.spec_reload_tx.write().await = Some(tx);
        rx
    }

    /// Subscribe to workspace changes for `tenant_id`.
    ///
    /// Creates a per-tenant broadcast channel on first call for that tenant.
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

    /// Spawn a background task that listens on `workspace_changes` and
    /// dispatches events to the appropriate per-tenant channel.
    fn spawn_listener(self: Arc<Self>) {
        tokio::spawn(async move {
            loop {
                match self.run_listener_loop().await {
                    Ok(()) => {
                        info!("realtime listener exited cleanly");
                        break;
                    }
                    Err(e) => {
                        error!(error = %e, "realtime listener error — reconnecting in 5s");
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    }
                }
            }
        });
    }

    async fn run_listener_loop(&self) -> anyhow::Result<()> {
        let mut listener = sqlx::postgres::PgListener::connect_with(&self.pool).await?;
        listener.listen("workspace_changes").await?;
        listener.listen("capability_specs_changed").await?;
        info!(
            "realtime listener connected to workspace_changes + capability_specs_changed channels"
        );

        loop {
            let notification = listener.recv().await?;
            let payload = notification.payload();
            let channel = notification.channel();
            match channel {
                "workspace_changes" => {
                    match serde_json::from_str::<WorkspaceChangeEvent>(payload) {
                        Ok(event) => {
                            let channels = self.channels.read().await;
                            if let Some(tx) = channels.get(&event.tenant_id) {
                                let _ = tx.send(event);
                            }
                        }
                        Err(e) => {
                            warn!(error = %e, payload, "failed to deserialise workspace_changes payload");
                        }
                    }
                }
                "capability_specs_changed" => {
                    // Forward to registered capability-spec reload handlers.
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(payload) {
                        let ns = v["namespace"].as_str().unwrap_or("").to_string();
                        let tn = v["tool_name"].as_str().unwrap_or("").to_string();
                        let tx_guard = self.spec_reload_tx.read().await;
                        if let Some(tx) = tx_guard.as_ref() {
                            let _ = tx.send((ns, tn));
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
