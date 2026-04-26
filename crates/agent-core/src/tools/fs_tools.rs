use anyhow::Result;
use common::path_safety::safe_join;
use serde_json::{json, Value};
use std::path::Path;

pub async fn read_file(workspace_root: &str, input: &Value) -> Result<Value> {
    let rel = input["path"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing required field: path"))?;

    let full = safe_join(Path::new(workspace_root), rel)
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let content = tokio::fs::read_to_string(&full)
        .await
        .map_err(|e| anyhow::anyhow!("read_file {rel}: {e}"))?;

    Ok(json!({ "path": rel, "content": content }))
}

pub async fn write_file(workspace_root: &str, input: &Value) -> Result<Value> {
    let rel = input["path"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing required field: path"))?;
    let content = input["content"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing required field: content"))?;

    let full = safe_join(Path::new(workspace_root), rel)
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    if let Some(parent) = full.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| anyhow::anyhow!("create_dir {rel}: {e}"))?;
    }

    tokio::fs::write(&full, content)
        .await
        .map_err(|e| anyhow::anyhow!("write_file {rel}: {e}"))?;

    Ok(json!({ "path": rel, "bytes_written": content.len() }))
}
