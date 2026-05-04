use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ulid::Ulid;

/// A single immutable audit record. Append-only — never updated after creation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: String,
    pub tenant_id: String,
    pub timestamp: DateTime<Utc>,
    pub action: String,
    /// Tool or capability name, if applicable (e.g. "invoice-processing__extract_invoice").
    pub tool: Option<String>,
    /// HTTP status or domain-level outcome ("ok", "error", "rate_limited").
    pub status: String,
    /// Round-trip latency in milliseconds.
    pub duration_ms: Option<u64>,
    /// Free-form context (model ID, file name, error message, etc.).
    pub metadata: serde_json::Value,
}

impl AuditEvent {
    pub fn new(tenant_id: impl Into<String>, action: impl Into<String>) -> Self {
        Self {
            id: Ulid::new().to_string(),
            tenant_id: tenant_id.into(),
            timestamp: Utc::now(),
            action: action.into(),
            tool: None,
            status: "ok".into(),
            duration_ms: None,
            metadata: serde_json::Value::Null,
        }
    }

    pub fn with_tool(mut self, tool: impl Into<String>) -> Self {
        self.tool = Some(tool.into());
        self
    }

    pub fn with_status(mut self, status: impl Into<String>) -> Self {
        self.status = status.into();
        self
    }

    pub fn with_duration_ms(mut self, ms: u64) -> Self {
        self.duration_ms = Some(ms);
        self
    }

    pub fn with_metadata(mut self, meta: serde_json::Value) -> Self {
        self.metadata = meta;
        self
    }
}

#[async_trait]
pub trait AuditStore: Send + Sync + 'static {
    /// Append an event. Fire-and-forget: implementations should not block callers on errors.
    async fn append(&self, event: AuditEvent) -> crate::error::Result<()>;

    /// Retrieve recent events for a tenant, newest first. `limit` caps the result set.
    /// `after` is an opaque cursor (event `id`) — only events older than that event are returned.
    async fn list(
        &self,
        tenant_id: &str,
        limit: usize,
        after: Option<&str>,
    ) -> crate::error::Result<Vec<AuditEvent>>;
}
