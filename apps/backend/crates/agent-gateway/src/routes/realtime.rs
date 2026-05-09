//! WebSocket endpoint for real-time workspace change events.
//!
//! `GET /api/realtime/workspace?tenant_id=<id>` upgrades to a WebSocket
//! connection.  The client receives `WorkspaceChangeEvent` JSON messages
//! whenever a workspace node is inserted, updated, or deleted within the
//! specified tenant.

use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use axum::{
    extract::ws::{Message, WebSocket},
    extract::{Extension, Query, State, WebSocketUpgrade},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;
use std::sync::Arc;
use tracing::{debug, warn};

#[derive(Deserialize)]
pub struct RealtimeQuery {
    pub tenant_id: Option<String>,
}

/// WebSocket handler — upgrades and starts forwarding events.
pub async fn realtime_workspace(
    ws: WebSocketUpgrade,
    Query(params): Query<RealtimeQuery>,
    Extension(tenant): Extension<ResolvedTenant>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if let Some(requested) = params.tenant_id
        && requested != tenant.0.tenant_id.to_string()
    {
        return StatusCode::FORBIDDEN.into_response();
    }

    let tenant_id = tenant.0.tenant_id.to_string();
    ws.on_upgrade(move |socket| handle_socket(socket, tenant_id, state))
}

async fn handle_socket(mut socket: WebSocket, tenant_id: String, state: Arc<AppState>) {
    let Some(realtime) = &state.realtime_service else {
        warn!("realtime service not available (test mode) — closing WebSocket");
        let _ = socket.send(Message::Close(None)).await;
        return;
    };

    let mut rx = realtime.subscribe_workspace(&tenant_id).await;
    debug!(tenant_id, "WebSocket client connected to workspace_changes");

    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(event) => {
                        match serde_json::to_string(&event) {
                            Ok(json) => {
                                if socket.send(Message::Text(json.into())).await.is_err() {
                                    // Client disconnected.
                                    break;
                                }
                            }
                            Err(e) => {
                                warn!(error = %e, "failed to serialise WorkspaceChangeEvent");
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        // Channel shut down.
                        break;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        warn!(tenant_id, missed = n, "WebSocket client lagged — missed {n} events");
                        // Continue rather than disconnect.
                    }
                }
            }
            msg = socket.recv() => {
                // Client sent something or closed the connection.
                if msg.is_none() {
                    break;
                }
            }
        }
    }

    debug!(tenant_id, "WebSocket client disconnected");
}
