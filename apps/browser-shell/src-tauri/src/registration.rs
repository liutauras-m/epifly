//! Session-trace upload to the gateway with the shell's bearer token.
//!
//! The shell used to self-register a `trace.replay` remote-MCP capability with
//! a device token on startup. The gateway no longer accepts device tokens or
//! self-registration, so that path is gone; nothing ran the replay anyway.

use common::trace::SessionTrace;

/// Uploads a completed SessionTrace to the workspace as a file node.
///
/// Steps (per plan §4.6):
///
///   1. Serialize trace → JSON bytes
///   2. POST /v1/files (multipart) → FileToken
///   3. POST /v1/workspaces to create a file workspace node
///
/// Returns the workspace node id.
pub async fn upload_trace(
    api_base: &str,
    access_token: &str,
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
        .bearer_auth(access_token)
        .multipart(form)
        .send()
        .await?;

    if !file_res.status().is_success() {
        let status = file_res.status();
        let body = file_res.text().await.unwrap_or_default();
        anyhow::bail!("file upload failed {status}: {body}");
    }

    let file_token: serde_json::Value = file_res.json().await?;
    let file_token_id = file_token["id"].as_str().unwrap_or_default().to_owned();

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
        .bearer_auth(access_token)
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
pub async fn upload_trace_cmd<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    webview: tauri::Webview<R>,
    auth_state: tauri::State<'_, crate::auth::AuthState>,
    trace: SessionTrace,
) -> Result<String, String> {
    if webview.label() != "main" {
        return Err("trace upload is only available to the shell window".into());
    }
    let api_base = crate::auth::gateway_url()?;
    let token = auth_state.access_token(&app).await?;
    upload_trace(api_base, &token, &trace)
        .await
        .map_err(|e| e.to_string())
}
