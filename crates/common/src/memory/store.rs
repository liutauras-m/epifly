use super::thread::{Message, Thread};
use async_trait::async_trait;

/// Pluggable persistent thread store.
///
/// Default implementation: `QdrantThreadStore` (agent-core crate).
/// Future: Redis, SurrealDB, or in-memory (tests).
#[async_trait]
pub trait ThreadStore: Send + Sync + 'static {
    /// Create a new thread, optionally seeding it with initial messages.
    async fn create(
        &self,
        tenant_id: &str,
        initial_messages: Vec<Message>,
    ) -> anyhow::Result<Thread>;

    /// Fetch thread metadata (no messages).
    async fn get(&self, tenant_id: &str, thread_id: &str) -> anyhow::Result<Option<Thread>>;

    /// Retrieve messages for a thread, ordered by seq ascending.
    async fn messages(&self, tenant_id: &str, thread_id: &str) -> anyhow::Result<Vec<Message>>;

    /// Append a single message; updates last_active + message_count on the thread doc.
    async fn append(
        &self,
        tenant_id: &str,
        thread_id: &str,
        message: Message,
    ) -> anyhow::Result<()>;

    /// List threads for a tenant, newest first.
    async fn list(&self, tenant_id: &str, limit: usize) -> anyhow::Result<Vec<Thread>>;

    /// Persist a generated summary (called by the background summariser).
    async fn set_summary(
        &self,
        tenant_id: &str,
        thread_id: &str,
        summary: String,
    ) -> anyhow::Result<()>;

    /// Persist an auto-generated title.
    async fn set_title(
        &self,
        tenant_id: &str,
        thread_id: &str,
        title: String,
    ) -> anyhow::Result<()>;
}
