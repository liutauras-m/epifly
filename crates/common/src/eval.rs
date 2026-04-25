use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalSample {
    pub id: String,
    pub input: serde_json::Value,
    pub expected: serde_json::Value,
    pub metadata: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalResult {
    pub sample_id: String,
    pub score: f64,
    pub passed: bool,
    pub details: serde_json::Value,
}

#[async_trait]
pub trait EvalRunner: Send + Sync {
    async fn run(&self, sample: &EvalSample) -> crate::error::Result<EvalResult>;
}

#[async_trait]
pub trait EvalScorer: Send + Sync {
    async fn score(&self, result: &EvalResult) -> crate::error::Result<f64>;
}
