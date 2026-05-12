use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    AgentTurn,
    CapabilityInvoke,
    Token,
    StorageGb,
    FileUpload,
}

impl std::fmt::Display for ActionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActionType::AgentTurn => write!(f, "agent_turn"),
            ActionType::CapabilityInvoke => write!(f, "capability_invoke"),
            ActionType::Token => write!(f, "token"),
            ActionType::StorageGb => write!(f, "storage_gb"),
            ActionType::FileUpload => write!(f, "file_upload"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageEvent {
    pub transaction_id: String,
    pub tenant_id: String,
    pub lago_customer_id: String,
    pub action: ActionType,
    pub quantity: u64,
    pub timestamp: DateTime<Utc>,
    pub properties: serde_json::Value,
}

impl UsageEvent {
    pub fn new(
        tenant_id: impl Into<String>,
        lago_customer_id: impl Into<String>,
        action: ActionType,
        quantity: u64,
    ) -> Self {
        Self {
            transaction_id: uuid::Uuid::new_v4().to_string(),
            tenant_id: tenant_id.into(),
            lago_customer_id: lago_customer_id.into(),
            action,
            quantity,
            timestamp: Utc::now(),
            properties: serde_json::Value::Object(Default::default()),
        }
    }

    pub fn with_properties(mut self, props: serde_json::Value) -> Self {
        self.properties = props;
        self
    }
}
