use common::trace::SessionTrace;

/// Posts the static capability manifest to the backend on first run.
pub async fn register_capability(api_base: &str, device_token: &str) -> anyhow::Result<()> {
    let manifest = serde_json::json!({
        "capability_id": "trace.replay",
        "kind": "remote_mcp",
        "endpoint": "ws://localhost:0/unused",
        "tools": [{
            "name": "replay_session",
            "description": "Replay a recorded SessionTrace as a deterministic plan",
            "input_schema": {
                "type": "object",
                "properties": {
                    "trace_node_id": { "type": "string" },
                    "dry_run": { "type": "boolean" }
                },
                "required": ["trace_node_id"]
            }
        }],
        "tenant_scope": []
    });

    let client = reqwest::Client::new();
    let res = client
        .post(format!("{api_base}/admin/capabilities/register"))
        .header("X-Device-Token", device_token)
        .json(&manifest)
        .send()
        .await?;

    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        anyhow::bail!("capability registration failed {status}: {body}");
    }
    Ok(())
}

/// Uploads a completed SessionTrace to the workspace as a file node.
///
/// Steps (per plan §4.6):
///   1. Serialize trace → JSON bytes
///   2. POST /v1/files (multipart) → FileToken
///   3. POST /v1/workspaces to create a file workspace node
///   Returns the workspace node id.
pub async fn upload_trace(
    api_base: &str,
    device_token: &str,
    trace: &SessionTrace,
) -> anyhow::Result<String> {
    let json_bytes = serde_json::to_vec(trace)?;
    let trace_id = trace.id.clone();
    let filename = format!("session-{}.trace.json", trace_id);
    let urls_json = serde_json::to_string(&trace.urls)?;

    let client = reqwest::Client::new();

    // 1. Upload the trace JSON as a file.
    let part = reqwest::multipart::Part::bytes(json_bytes)
        .file_name(filename.clone())
        .mime_str("application/json")?;
    let form = reqwest::multipart::Form::new().part("file", part);

    let file_res = client
        .post(format!("{api_base}/v1/files"))
        .header("X-Device-Token", device_token)
        .multipart(form)
        .send()
        .await?;

    if !file_res.status().is_success() {
        let status = file_res.status();
        let body = file_res.text().await.unwrap_or_default();
        anyhow::bail!("file upload failed {status}: {body}");
    }

    let file_token: serde_json::Value = file_res.json().await?;
    let file_token_id = file_token["id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();

    // 2. Create a workspace node pointing at the uploaded file.
    let node_body = serde_json::json!({
        "kind": "file",
        "name": filename,
        "file_token": file_token_id,
        "metadata": {
            "source": "browser-shell",
            "trace_id": trace_id,
            "urls": urls_json,
        }
    });

    let ws_res = client
        .post(format!("{api_base}/v1/workspaces"))
        .header("X-Device-Token", device_token)
        .json(&node_body)
        .send()
        .await?;

    if !ws_res.status().is_success() {
        let status = ws_res.status();
        let body = ws_res.text().await.unwrap_or_default();
        anyhow::bail!("workspace node creation failed {status}: {body}");
    }

    let node: serde_json::Value = ws_res.json().await?;
    Ok(node["id"].as_str().unwrap_or_default().to_owned())
}

/// Tauri command: upload a completed trace and return the workspace node id.
#[tauri::command]
pub async fn upload_trace_cmd(
    token_state: tauri::State<'_, crate::device_auth::DeviceAuthHandle>,
    trace: SessionTrace,
) -> Result<String, String> {
    use crate::device_auth::DeviceTokenProvider;
    let api_base = std::env::var("CONUSAI_API_BASE")
        .unwrap_or_else(|_| "http://localhost:8080".to_owned());
    let token = token_state
        .token()
        .ok_or("no device token — cannot upload trace")?;
    upload_trace(&api_base, &token, &trace)
        .await
        .map_err(|e| e.to_string())
}
