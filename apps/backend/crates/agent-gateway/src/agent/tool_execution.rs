//! Tool execution helpers — Step 2.4.
//!
//! Moved from `routes/agent.rs`; updated to use `registry.read()`.

use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use common::metrics;
use serde_json::Value;
use std::sync::Arc;
use tracing::debug;

// ── truncate_tool_result ──────────────────────────────────────────────────────

/// Truncate a tool result to `max_bytes` bytes (byte boundary, not char boundary).
///
/// On truncation, appends `\n…[truncated N bytes]` and emits the
/// `tool_result_truncated` metric so the event is observable (Step 1.6).
pub fn truncate_tool_result(content: String, tool_name: &str, max_bytes: usize) -> String {
    if content.len() <= max_bytes {
        return content;
    }
    let overflow = content.len() - max_bytes;
    let boundary = content
        .char_indices()
        .map(|(i, _)| i)
        .take_while(|&i| i <= max_bytes)
        .last()
        .unwrap_or(0);
    let truncated = format!(
        "{}\n\u{2026}[truncated {} bytes]",
        &content[..boundary],
        overflow,
    );
    metrics::record_tool_result_truncated(tool_name);
    truncated
}

// ── maybe_inject_current_content ─────────────────────────────────────────────

/// PR 2.D — read-before-write injection.
///
/// If the tool's manifest declares `read_before_write = "<field>"`, read the
/// file at `input[field]` from `WorkspaceContentStore` and inject
/// `_current_content` (and `_is_new_file`) into the input before the tool runs.
/// Returns `Some(patched_input)` when injection happened, `None` otherwise.
pub async fn maybe_inject_current_content(
    state: &Arc<AppState>,
    full_tool_name: &str,
    input: &Value,
    tenant_id: &str,
) -> Option<Value> {
    let (safe_cap, tool_name) = full_tool_name.split_once("__")?;

    let (_field, path) = {
        let cap_name_dot = safe_cap.replace('_', ".");
        let registry = state.registry.read();
        let card = registry
            .get(safe_cap)
            .or_else(|| registry.get(&cap_name_dot))
            .cloned()?;
        let tool_def = card.manifest.tools.iter().find(|t| t.name == tool_name)?;
        let field = tool_def.read_before_write.clone()?;
        let path = input.get(&field)?.as_str()?.to_owned();
        if path.is_empty() {
            return None;
        }
        (field, path)
    };

    let mut patched = input.clone();
    match state.workspace_content.read(tenant_id, &path, None).await {
        Ok(content) => {
            patched["_current_content"] = serde_json::json!(content);
            patched["_is_new_file"] = serde_json::json!(false);
            debug!(
                tool = full_tool_name,
                path,
                "read_before_write: injected _current_content ({} bytes)",
                content.len()
            );
        }
        Err(_) => {
            patched["_current_content"] = serde_json::Value::Null;
            patched["_is_new_file"] = serde_json::json!(true);
            debug!(
                tool = full_tool_name,
                path, "read_before_write: file not found, injecting _is_new_file=true"
            );
        }
    }
    Some(patched)
}

// ── resolve_and_invoke ────────────────────────────────────────────────────────

/// Returns `(tool_output_value, changed_virtual_paths)`.
/// `changed_virtual_paths` is non-empty when the tool materialised workspace artifacts (PR 3.A).
pub async fn resolve_and_invoke(
    state: &Arc<AppState>,
    full_tool_name: &str,
    input: &Value,
    tenant: &ResolvedTenant,
) -> anyhow::Result<(Value, Vec<String>)> {
    let injected =
        maybe_inject_current_content(state, full_tool_name, input, &tenant.0.tenant_id).await;
    let effective_input = injected.as_ref().unwrap_or(input);

    let mut raw_result = state
        .semantic_router
        .invoke(full_tool_name, effective_input, Some(&tenant.0))
        .await?;

    let mut changed_paths: Vec<String> = vec![];

    if let Some(ref bridge) = state.artifact_bridge
        && let Ok(tool_out) =
            serde_json::from_value::<common::artifact::ToolOutput>(raw_result.clone())
        && !tool_out.artifacts.is_empty()
    {
        let tool_short = full_tool_name.split("__").next().unwrap_or(full_tool_name);
        if let Ok((public_url, paths)) = bridge
            .process_if_artifacts(
                &tenant.0.tenant_id,
                tenant.0.user_id.as_deref(),
                tool_short,
                None,
                &tool_out,
            )
            .await
        {
            changed_paths = paths;
            if let Some(url) = public_url
                && let Some(obj) = raw_result.as_object_mut()
            {
                obj.insert("public_url".to_string(), serde_json::json!(url));
            }
        }
    }

    const STORAGE_WS_PREFIX: &str = "storage-workspace__";
    const STORAGE_WS_READONLY: &[&str] = &[
        "storage-workspace__list_folders",
        "storage-workspace__show_tree",
        "storage-workspace__find_by_name",
    ];
    if changed_paths.is_empty()
        && full_tool_name.starts_with(STORAGE_WS_PREFIX)
        && !STORAGE_WS_READONLY.contains(&full_tool_name)
    {
        changed_paths.push("*".to_string());
    }

    Ok((raw_result, changed_paths))
}
