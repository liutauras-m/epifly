use common::mcp::{JsonRpcRequest, JsonRpcResponse};
use reqwest::Client;
use serde_json::Value;
use tracing::{Span, instrument};

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

    #[instrument(skip(self, params), fields(mcp.endpoint = %self.endpoint, mcp.method = method, error.type = tracing::field::Empty))]
    pub async fn call(&self, method: &str, params: Option<Value>) -> common::error::Result<Value> {
        let req = JsonRpcRequest::new(method, params);
        let result = self
            .client
            .post(&self.endpoint)
            .json(&req)
            .send()
            .await
            .map_err(|e| common::error::ConusAiError::Mcp(e.to_string()))
            .and_then(|r| {
                // We need async, so we'll do it below
                Ok(r)
            });

        let resp: JsonRpcResponse = result?
            .json()
            .await
            .map_err(|e| common::error::ConusAiError::Mcp(e.to_string()))?;

        if let Some(err) = resp.error {
            let msg = format!("MCP error {}: {}", err.code, err.message);
            Span::current().record("error.type", &msg);
            tracing::error!(error = %msg, "MCP JSON-RPC error");
            return Err(common::error::ConusAiError::Mcp(msg));
        }

        Ok(resp.result.unwrap_or(Value::Null))
    }

    #[instrument(skip(self), fields(mcp.endpoint = %self.endpoint))]
    pub async fn list_tools(&self) -> common::error::Result<Value> {
        self.call("tools/list", None).await
    }

    #[instrument(skip(self, args), fields(mcp.endpoint = %self.endpoint, tool.name = name, error.type = tracing::field::Empty))]
    pub async fn call_tool(&self, name: &str, args: Value) -> common::error::Result<Value> {
        use serde_json::json;
        let result = self
            .call("tools/call", Some(json!({"name": name, "arguments": args})))
            .await;
        if let Err(ref e) = result {
            Span::current().record("error.type", e.to_string().as_str());
        }
        result
    }
}
