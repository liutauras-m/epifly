//! WebSocket control channel for browser-shell clients.
//!
//! `GET /v1/shells/{device_id}/control` — bidirectional WS:
//!   - Client sends: `{ "kind": "Heartbeat", "payload": null }`
//!   - Server sends: `{ "kind": "Replay", "payload": { "trace_node_id": "…", "dry_run": true } }`

use crate::routes::admin_devices::{require_shell_feature, validate_device_token};
use crate::state::AppState;
use axum::{
    extract::{Path, Query, State, WebSocketUpgrade, ws::{Message, WebSocket}},
    response::IntoResponse,
};
use common::error::HttpError;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn};

#[derive(Debug, Deserialize)]
pub struct ShellQuery {
    pub device_token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ControlMessage {
    pub kind: ControlKind,
    pub payload: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ControlKind {
    Heartbeat,
    Replay,
    Stop,
    Ack,
}

pub async fn shell_control(
    State(state): State<Arc<AppState>>,
    Path(device_id): Path<String>,
    Query(query): Query<ShellQuery>,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, HttpError> {
    require_shell_feature()?;
    let pool = state
        .pool
        .as_ref()
        .ok_or_else(|| HttpError::internal("no database pool", None))?
        .clone();

    let token = query
        .device_token
        .as_deref()
        .unwrap_or("")
        .to_owned();

    Ok(ws.on_upgrade(move |socket| handle_shell_ws(socket, device_id, token, pool)))
}

async fn handle_shell_ws(
    socket: WebSocket,
    device_id: String,
    device_token: String,
    pool: sqlx::PgPool,
) {
    // Authenticate immediately; close if invalid.
    match validate_device_token(&pool, &device_token).await {
        Ok(Some(tenant_id)) => {
            info!(device_id, tenant_id, "shell connected");
        }
        Ok(None) => {
            warn!(device_id, "shell rejected: invalid or revoked token");
            return;
        }
        Err(e) => {
            warn!(device_id, err = %e, "device token validation error");
            return;
        }
    }

    // Read per-turn replay quota from env (default 3).
    let max_replays: u32 = std::env::var("CONUSAI_MAX_REPLAYS_PER_TURN")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3);

    let (mut sender, mut receiver) = socket.split();
    let mut replays_this_turn: u32 = 0;

    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            Message::Text(text) => {
                match serde_json::from_str::<ControlMessage>(&text) {
                    Ok(ControlMessage { kind: ControlKind::Heartbeat, .. }) => {
                        // Reset per-turn replay counter on each heartbeat.
                        replays_this_turn = 0;
                        let ack = serde_json::to_string(&ControlMessage {
                            kind: ControlKind::Ack,
                            payload: serde_json::Value::Null,
                        })
                        .unwrap_or_default();
                        let _ = sender.send(Message::Text(ack.into())).await;
                    }
                    Ok(ControlMessage { kind: ControlKind::Replay, payload }) => {
                        replays_this_turn += 1;
                        if replays_this_turn > max_replays {
                            warn!(
                                device_id,
                                limit = max_replays,
                                "replay quota exceeded; closing connection"
                            );
                            let stop = serde_json::to_string(&ControlMessage {
                                kind: ControlKind::Stop,
                                payload: serde_json::json!({ "reason": "replay_quota_exceeded" }),
                            })
                            .unwrap_or_default();
                            let _ = sender.send(Message::Text(stop.into())).await;
                            let _ = sender.send(Message::Close(None)).await;
                            break;
                        }
                        info!(device_id, replay_n = replays_this_turn, payload = ?payload, "received replay message");
                    }
                    Ok(msg) => {
                        info!(device_id, kind = ?msg.kind, "received shell message");
                    }
                    Err(e) => {
                        warn!(device_id, err = %e, "malformed shell message");
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    info!(device_id, "shell disconnected");
}
