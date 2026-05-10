/// Posts the static capability manifest to the backend on first run.
/// Authenticated via the device token stored in Stronghold.
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
