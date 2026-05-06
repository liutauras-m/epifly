use async_trait::async_trait;
use common::audit::{AuditEvent, AuditStore};
use sqlx::PgPool;
use tracing::instrument;

pub struct PostgresAuditStore {
    pool: PgPool,
}

impl PostgresAuditStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AuditStore for PostgresAuditStore {
    #[instrument(skip(self, event), fields(tenant_id = %event.tenant_id, action = %event.action))]
    async fn append(&self, event: AuditEvent) -> common::error::Result<()> {
        let metadata_val = serde_json::to_value(&event.metadata).unwrap_or(serde_json::Value::Null);

        sqlx::query!(
            "INSERT INTO audit_events (id, tenant_id, timestamp, action, tool, status, duration_ms, metadata)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT (id) DO NOTHING",
            event.id,
            event.tenant_id,
            event.timestamp,
            event.action,
            event.tool,
            event.status,
            event.duration_ms.map(|d| d as i32),
            metadata_val,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| common::error::ConusAiError::Database(e.to_string()))?;

        Ok(())
    }

    #[instrument(skip(self), fields(tenant_id, limit))]
    async fn list(
        &self,
        tenant_id: &str,
        limit: usize,
        after: Option<&str>,
    ) -> common::error::Result<Vec<AuditEvent>> {
        let rows: Vec<AuditEvent> = if let Some(cursor) = after {
            // Cursor: return events older than the event identified by `cursor`
            let cursor_ts =
                sqlx::query_scalar!("SELECT timestamp FROM audit_events WHERE id = $1", cursor,)
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(|e| common::error::ConusAiError::Database(e.to_string()))?;

            if let Some(ts) = cursor_ts {
                sqlx::query!(
                    "SELECT id, tenant_id, timestamp, action, tool, status, duration_ms, metadata
                     FROM audit_events
                     WHERE tenant_id = $1 AND timestamp < $2
                     ORDER BY timestamp DESC
                     LIMIT $3",
                    tenant_id,
                    ts,
                    limit as i64,
                )
                .fetch_all(&self.pool)
                .await
                .map_err(|e| common::error::ConusAiError::Database(e.to_string()))?
                .into_iter()
                .map(|r| AuditEvent {
                    id: r.id,
                    tenant_id: r.tenant_id,
                    timestamp: r.timestamp,
                    action: r.action,
                    tool: r.tool,
                    status: r.status,
                    duration_ms: r.duration_ms.map(|d| d as u64),
                    metadata: r.metadata,
                })
                .collect()
            } else {
                vec![]
            }
        } else {
            sqlx::query!(
                "SELECT id, tenant_id, timestamp, action, tool, status, duration_ms, metadata
                 FROM audit_events
                 WHERE tenant_id = $1
                 ORDER BY timestamp DESC
                 LIMIT $2",
                tenant_id,
                limit as i64,
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|e| common::error::ConusAiError::Database(e.to_string()))?
            .into_iter()
            .map(|r| AuditEvent {
                id: r.id,
                tenant_id: r.tenant_id,
                timestamp: r.timestamp,
                action: r.action,
                tool: r.tool,
                status: r.status,
                duration_ms: r.duration_ms.map(|d| d as u64),
                metadata: r.metadata,
            })
            .collect()
        };

        Ok(rows)
    }
}
