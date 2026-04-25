use async_trait::async_trait;
use serde_json::Value;

#[async_trait]
pub trait AgentCapability: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn tool_names(&self) -> Vec<String>;
    async fn invoke(&self, tool: &str, input: Value) -> common::error::Result<Value>;
}
