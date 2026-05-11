use futures_util::StreamExt;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};
use tokio::task::JoinHandle;

pub type StreamRegistry = Arc<Mutex<HashMap<String, JoinHandle<()>>>>;

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChunkPayload {
    Text { content: String },
    ToolStart { id: String, name: String },
    ToolResult { tool_use_id: String, result: String },
    ThreadId { id: String },
    Done,
    Error { message: String },
}

#[tauri::command]
pub async fn chat_stream_start(
    app: AppHandle,
    registry: State<'_, StreamRegistry>,
    message: String,
    session_token: String,
    thread_id: Option<String>,
    workspace_node_id: Option<String>,
    api_base: String,
) -> Result<String, String> {
    let stream_id = ulid::Ulid::new().to_string();
    let sid = stream_id.clone();

    let handle = tokio::spawn(async move {
        let client = reqwest::Client::new();
        let mut body = serde_json::json!({ "message": message });
        if let Some(tid) = thread_id {
            body["thread_id"] = serde_json::json!(tid);
        }
        if let Some(nid) = workspace_node_id {
            body["workspace_node_id"] = serde_json::json!(nid);
        }

        let result = client
            .post(format!("{}/ui/stream", api_base))
            .header("Content-Type", "application/json")
            .header("X-Session-Token", &session_token)
            .json(&body)
            .send()
            .await;

        let response = match result {
            Err(e) => {
                let _ = app.emit(
                    &format!("chat:chunk:{}", sid),
                    ChunkPayload::Error {
                        message: e.to_string(),
                    },
                );
                return;
            }
            Ok(r) if !r.status().is_success() => {
                let status = r.status().as_u16();
                let body = r.text().await.unwrap_or_default();
                let _ = app.emit(
                    &format!("chat:chunk:{}", sid),
                    ChunkPayload::Error {
                        message: format!("HTTP {}: {}", status, body),
                    },
                );
                return;
            }
            Ok(r) => r,
        };

        let mut byte_stream = response.bytes_stream();
        let mut buf = String::new();

        while let Some(chunk) = byte_stream.next().await {
            let bytes = match chunk {
                Err(e) => {
                    let _ = app.emit(
                        &format!("chat:chunk:{}", sid),
                        ChunkPayload::Error {
                            message: e.to_string(),
                        },
                    );
                    return;
                }
                Ok(b) => b,
            };
            buf.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(pos) = buf.find("\n\n") {
                let block = buf[..pos].to_string();
                buf = buf[pos + 2..].to_string();

                for line in block.lines() {
                    let Some(data) = line.strip_prefix("data: ") else {
                        continue;
                    };
                    if data == "[DONE]" {
                        let _ = app.emit(&format!("chat:chunk:{}", sid), ChunkPayload::Done);
                        return;
                    }
                    let Ok(ev) = serde_json::from_str::<serde_json::Value>(data) else {
                        continue;
                    };

                    let delta = ev["choices"][0]["delta"].as_object();
                    if let Some(d) = delta {
                        if let Some(content) = d.get("content").and_then(|v| v.as_str()) {
                            let _ = app.emit(
                                &format!("chat:chunk:{}", sid),
                                ChunkPayload::Text {
                                    content: content.to_string(),
                                },
                            );
                        } else if let Some(ts) = d.get("tool_call_start") {
                            let id = ts["id"].as_str().unwrap_or("").to_string();
                            let name = ts["name"].as_str().unwrap_or("").to_string();
                            let _ = app.emit(
                                &format!("chat:chunk:{}", sid),
                                ChunkPayload::ToolStart { id, name },
                            );
                        } else if let Some(tr) = d.get("tool_call_result") {
                            let tool_use_id = tr["tool_use_id"].as_str().unwrap_or("").to_string();
                            let result = tr["result"].as_str().unwrap_or("").to_string();
                            let _ = app.emit(
                                &format!("chat:chunk:{}", sid),
                                ChunkPayload::ToolResult {
                                    tool_use_id,
                                    result,
                                },
                            );
                        }
                    }

                    if let Some(tid) = ev["thread_id"].as_str() {
                        let _ = app.emit(
                            &format!("chat:chunk:{}", sid),
                            ChunkPayload::ThreadId {
                                id: tid.to_string(),
                            },
                        );
                    }
                }
            }
        }
        // Stream ended without [DONE] — emit done anyway
        let _ = app.emit(&format!("chat:chunk:{}", sid), ChunkPayload::Done);
    });

    registry.lock().unwrap().insert(stream_id.clone(), handle);
    Ok(stream_id)
}

#[tauri::command]
pub async fn chat_stream_abort(
    registry: State<'_, StreamRegistry>,
    stream_id: String,
) -> Result<(), String> {
    if let Some(handle) = registry.lock().unwrap().remove(&stream_id) {
        handle.abort();
    }
    Ok(())
}
