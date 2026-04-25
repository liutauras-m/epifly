use common::mcp::{JsonRpcRequest, JsonRpcResponse};
use reqwest::Client;
use serde_json::Value;

pub struct McpAdapter {
    endpoint: String,
    client: Client,
}

impl McpAdapter {
    pub fn new(endpoint: impl Into<String>) -> common::error::Result<Self> {
        Ok(Self {
            endpoint: endpoint.into(),
            client: common::http_client::build_client()?,
        })
    }

    pub async fn call(&self, method: &str, params: Option<Value>) -> common::error::Result<Value> {
        let req = JsonRpcRequest::new(method, params);
        let resp: JsonRpcResponse = self
            .client
            .post(&self.endpoint)
            .json(&req)
            .send()
            .await
            .map_err(|e| common::error::ConusAiError::Mcp(e.to_string()))?
            .json()
            .await
            .map_err(|e| common::error::ConusAiError::Mcp(e.to_string()))?;

        if let Some(err) = resp.error {
            return Err(common::error::ConusAiError::Mcp(format!(
                "MCP error {}: {}",
                err.code, err.message
            )));
        }

        Ok(resp.result.unwrap_or(Value::Null))
    }

    pub async fn list_tools(&self) -> common::error::Result<Value> {
        self.call("tools/list", None).await
    }

    pub async fn call_tool(&self, name: &str, args: Value) -> common::error::Result<Value> {
        use serde_json::json;
        self.call("tools/call", Some(json!({"name": name, "arguments": args})))
            .await
    }
}
