use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use agent_core::capabilities::executor::ToolExecutor;
use axum::{Extension, Json, extract::State};
use common::mcp::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::instrument;

/// POST /mcp — JSON-RPC 2.0 dispatcher (MCP protocol)
#[utoipa::path(
    post,
    path = "/mcp",
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "MCP JSON-RPC 2.0 response", body = serde_json::Value),
        (status = 429, description = "Rate limit exceeded"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer_auth" = []), ("api_key_auth" = [])),
    tag = "mcp",
)]
#[instrument(skip(state, tenant, req), fields(method = req.method.as_str()))]
pub async fn dispatch(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    Json(req): Json<JsonRpcRequest>,
) -> Json<JsonRpcResponse> {
    let id = req.id.clone();

    if !state
        .rate_limiter
        .check(&tenant.0.tenant_id, tenant.0.plan.limits().rate_limit_rpm)
    {
        return Json(JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code: 429,
                message: "rate limit exceeded".into(),
                data: None,
            }),
        });
    }

    let result = match req.method.as_str() {
        "initialize" => Ok(handle_initialize()),
        "tools/list" => Ok(handle_tools_list(&state)),
        "tools/call" => handle_tools_call(&state, &tenant, req.params.as_ref()).await,
        _ => Err(JsonRpcError {
            code: -32601,
            message: format!("Method not found: {}", req.method),
            data: None,
        }),
    };

    match result {
        Ok(val) => Json(JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id,
            result: Some(val),
            error: None,
        }),
        Err(err) => Json(JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(err),
        }),
    }
}

fn handle_initialize() -> Value {
    json!({
        "protocolVersion": "2024-11-05",
        "serverInfo": {
            "name": "conusai-platform",
            "version": "0.1.0"
        },
        "capabilities": {
            "tools": {}
        }
    })
}

fn handle_tools_list(state: &Arc<AppState>) -> Value {
    let registry = state.registry.lock().unwrap();
    let tools: Vec<Value> = registry
        .all_enabled()
        .flat_map(ToolExecutor::tool_definitions)
        .collect();
    json!({ "tools": tools })
}

async fn handle_tools_call(
    state: &Arc<AppState>,
    tenant: &ResolvedTenant,
    params: Option<&Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError {
        code: -32602,
        message: "Missing params".into(),
        data: None,
    })?;

    let name = params["name"].as_str().ok_or_else(|| JsonRpcError {
        code: -32602,
        message: "Missing tool name".into(),
        data: None,
    })?;

    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

    // Tool names are formatted as `capability_name__tool_name`
    let (cap_name, tool_name) = name.split_once("__").ok_or_else(|| JsonRpcError {
        code: -32602,
        message: format!("Invalid tool name '{name}': expected capability__tool"),
        data: None,
    })?;

    // Get provider Arc under a short-lived lock
    let provider = {
        let registry = state.registry.lock().unwrap();
        registry.get_provider(cap_name)
    }
    .ok_or_else(|| JsonRpcError {
        code: -32602,
        message: format!("Tool not found: {cap_name}"),
        data: None,
    })?;

    let result = provider
        .invoke(tool_name, &arguments, Some(&tenant.0))
        .await
        .map_err(|e| JsonRpcError {
            code: -32603,
            message: format!("Tool execution error: {e}"),
            data: None,
        })?;

    Ok(json!({
        "content": [{ "type": "text", "text": result.to_string() }],
        "isError": false
    }))
}
