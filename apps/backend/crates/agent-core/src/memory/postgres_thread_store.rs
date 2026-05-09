use async_trait::async_trait;
use chrono::Utc;
use common::memory::store::ThreadStore;
use common::memory::thread::{Message, Thread};
use common::types::ThreadId;
use sqlx::PgPool;
use tracing::instrument;
use ulid::Ulid;

pub struct PostgresThreadStore {
    pool: PgPool,
}

impl PostgresThreadStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ThreadStore for PostgresThreadStore {
    #[instrument(skip(self, initial_messages), fields(tenant_id))]
    async fn create(
        &self,
        tenant_id: &str,
        initial_messages: Vec<Message>,
    ) -> anyhow::Result<Thread> {
        let id = Ulid::new().to_string();
        let now = Utc::now();
        sqlx::query!(
            "INSERT INTO threads (id, tenant_id, created_at, last_active, message_count, metadata)
             VALUES ($1, $2, $3, $4, 0, '{}'::jsonb)",
            id,
            tenant_id,
            now,
            now,
        )
        .execute(&self.pool)
        .await?;

        for msg in initial_messages {
            self.append(tenant_id, &id, msg).await?;
        }

        self.get(tenant_id, &id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("thread not found after create"))
    }

    #[instrument(skip(self), fields(tenant_id, thread_id))]
    async fn get(&self, tenant_id: &str, thread_id: &str) -> anyhow::Result<Option<Thread>> {
        let row = sqlx::query!(
            "SELECT id, tenant_id, title, summary, created_at, last_active, message_count, metadata
             FROM threads WHERE id = $1 AND tenant_id = $2",
            thread_id,
            tenant_id,
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| Thread {
            id: r.id.parse::<ThreadId>().unwrap_or_else(|_| ThreadId::new()),
            tenant_id: r.tenant_id,
            title: r.title,
            summary: r.summary,
            created_at: r.created_at,
            last_active: r.last_active,
            message_count: r.message_count as usize,
            metadata: r.metadata,
        }))
    }

    #[instrument(skip(self), fields(tenant_id, thread_id))]
    async fn messages(&self, tenant_id: &str, thread_id: &str) -> anyhow::Result<Vec<Message>> {
        // Verify tenant ownership
        let exists = sqlx::query_scalar!(
            "SELECT 1 FROM threads WHERE id = $1 AND tenant_id = $2",
            thread_id,
            tenant_id,
        )
        .fetch_optional(&self.pool)
        .await?;

        if exists.is_none() {
            return Ok(vec![]);
        }

        let rows = sqlx::query!(
            "SELECT role, content, tool_calls, timestamp, seq
             FROM messages WHERE thread_id = $1 ORDER BY seq ASC",
            thread_id,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| Message {
                role: r.role,
                content: r.content,
                tool_calls: serde_json::from_value(r.tool_calls).ok().flatten(),
                timestamp: r.timestamp,
                seq: r.seq as usize,
            })
            .collect())
    }

    #[instrument(skip(self, message), fields(tenant_id, thread_id))]
    async fn append(
        &self,
        tenant_id: &str,
        thread_id: &str,
        message: Message,
    ) -> anyhow::Result<()> {
        let seq: i64 = sqlx::query_scalar!(
            "SELECT COALESCE(MAX(seq), -1) + 1 FROM messages WHERE thread_id = $1",
            thread_id,
        )
        .fetch_one(&self.pool)
        .await?
        .unwrap_or(0)
        .into();

        let tool_calls_val = serde_json::to_value(&message.tool_calls)?;

        sqlx::query!(
            "INSERT INTO messages (thread_id, seq, role, content, tool_calls, timestamp)
             VALUES ($1, $2, $3, $4, $5, $6)",
            thread_id,
            seq as i32,
            message.role,
            message.content,
            tool_calls_val,
            message.timestamp,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query!(
            "UPDATE threads SET message_count = message_count + 1, last_active = now()
             WHERE id = $1 AND tenant_id = $2",
            thread_id,
            tenant_id,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    #[instrument(skip(self), fields(tenant_id, limit))]
    async fn list(
        &self,
        tenant_id: &str,
        limit: usize,
        _after: Option<&str>,
    ) -> anyhow::Result<Vec<Thread>> {
        let rows = sqlx::query!(
            "SELECT id, tenant_id, title, summary, created_at, last_active, message_count, metadata
             FROM threads WHERE tenant_id = $1 ORDER BY last_active DESC LIMIT $2",
            tenant_id,
            limit as i64,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| Thread {
                id: r.id.parse::<ThreadId>().unwrap_or_else(|_| ThreadId::new()),
                tenant_id: r.tenant_id,
                title: r.title,
                summary: r.summary,
                created_at: r.created_at,
                last_active: r.last_active,
                message_count: r.message_count as usize,
                metadata: r.metadata,
            })
            .collect())
    }

    #[instrument(skip(self, summary), fields(tenant_id, thread_id))]
    async fn set_summary(
        &self,
        tenant_id: &str,
        thread_id: &str,
        summary: String,
    ) -> anyhow::Result<()> {
        sqlx::query!(
            "UPDATE threads SET summary = $1 WHERE id = $2 AND tenant_id = $3",
            summary,
            thread_id,
            tenant_id,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    #[instrument(skip(self, title), fields(tenant_id, thread_id))]
    async fn set_title(
        &self,
        tenant_id: &str,
        thread_id: &str,
        title: String,
    ) -> anyhow::Result<()> {
        sqlx::query!(
            "UPDATE threads SET title = $1 WHERE id = $2 AND tenant_id = $3",
            title,
            thread_id,
            tenant_id,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
