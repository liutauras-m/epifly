/// `ConversationService` — single source of truth for conversation history.
///
/// Replaces the dual-path pattern where `/v1/threads` and `workspace_node`
/// conversation binding each maintained independent persistence logic.
/// All thread creation and message appending now flows through this trait.
use crate::context::tenant::TenantContext;
use async_trait::async_trait;
use common::memory::store::{ThreadStore, WorkspaceStore};
use common::memory::thread::{Message, Thread};
use common::types::ThreadId;
use std::sync::Arc;
use tracing::{instrument, warn};
use ulid::Ulid;

#[async_trait]
pub trait ConversationService: Send + Sync + 'static {
    /// Create a new empty thread, optionally binding it to a workspace node.
    async fn create(
        &self,
        tenant: &TenantContext,
        node_id: Option<Ulid>,
    ) -> anyhow::Result<ThreadId>;

    /// Append a message to an existing thread.
    async fn append_message(
        &self,
        tenant: &TenantContext,
        thread_id: ThreadId,
        msg: Message,
    ) -> anyhow::Result<()>;

    /// Load full message history for a thread, ordered by seq ascending.
    async fn load_history(
        &self,
        tenant: &TenantContext,
        thread_id: ThreadId,
    ) -> anyhow::Result<Vec<Message>>;

    /// Resolve the thread bound to a workspace node, creating one lazily if absent.
    async fn resolve_for_node(
        &self,
        tenant: &TenantContext,
        node_id: Ulid,
    ) -> anyhow::Result<Option<ThreadId>>;

    /// List threads for the tenant (cursor-based pagination).
    async fn list(
        &self,
        tenant: &TenantContext,
        limit: usize,
        after: Option<&str>,
    ) -> anyhow::Result<Vec<Thread>>;

    /// Fetch a single thread (metadata, no messages).
    async fn get(
        &self,
        tenant: &TenantContext,
        thread_id: ThreadId,
    ) -> anyhow::Result<Option<Thread>>;
}

// ── Default implementation ────────────────────────────────────────────────────

pub struct DefaultConversationService {
    pub thread_store: Arc<dyn ThreadStore>,
    pub workspace_store: Arc<dyn WorkspaceStore>,
}

#[async_trait]
impl ConversationService for DefaultConversationService {
    #[instrument(skip(self, tenant), fields(tenant_id = %tenant.tenant_id))]
    async fn create(
        &self,
        tenant: &TenantContext,
        node_id: Option<Ulid>,
    ) -> anyhow::Result<ThreadId> {
        let thread = self.thread_store.create(&tenant.tenant_id, vec![]).await?;

        // Bind to workspace node lazily
        if let Some(nid) = node_id
            && let Err(e) = self
                .workspace_store
                .bind_thread(&tenant.tenant_id, nid, &thread.id.to_string())
                .await
        {
            warn!(error = %e, node_id = %nid, "failed to bind thread to workspace node");
        }

        Ok(thread.id)
    }

    #[instrument(skip(self, tenant, msg), fields(tenant_id = %tenant.tenant_id, %thread_id))]
    async fn append_message(
        &self,
        tenant: &TenantContext,
        thread_id: ThreadId,
        msg: Message,
    ) -> anyhow::Result<()> {
        self.thread_store
            .append(&tenant.tenant_id, &thread_id.to_string(), msg)
            .await
    }

    #[instrument(skip(self, tenant), fields(tenant_id = %tenant.tenant_id, %thread_id))]
    async fn load_history(
        &self,
        tenant: &TenantContext,
        thread_id: ThreadId,
    ) -> anyhow::Result<Vec<Message>> {
        self.thread_store
            .messages(&tenant.tenant_id, &thread_id.to_string())
            .await
    }

    #[instrument(skip(self, tenant), fields(tenant_id = %tenant.tenant_id, node_id = %node_id))]
    async fn resolve_for_node(
        &self,
        tenant: &TenantContext,
        node_id: Ulid,
    ) -> anyhow::Result<Option<ThreadId>> {
        // Check existing binding
        let node = self
            .workspace_store
            .get_accessible_node(
                &tenant.tenant_id,
                tenant.user_id.as_deref().unwrap_or("__dev__"),
                node_id,
            )
            .await?;

        if let Some(tid_str) = node.metadata.get("thread_id").and_then(|v| v.as_str())
            && let Ok(tid) = tid_str.parse::<ThreadId>()
        {
            return Ok(Some(tid));
        }

        // No existing thread — create and bind
        let tid = self.create(tenant, Some(node_id)).await?;
        Ok(Some(tid))
    }

    #[instrument(skip(self, tenant), fields(tenant_id = %tenant.tenant_id, limit, after))]
    async fn list(
        &self,
        tenant: &TenantContext,
        limit: usize,
        after: Option<&str>,
    ) -> anyhow::Result<Vec<Thread>> {
        self.thread_store
            .list(&tenant.tenant_id, limit, after)
            .await
    }

    #[instrument(skip(self, tenant), fields(tenant_id = %tenant.tenant_id, %thread_id))]
    async fn get(
        &self,
        tenant: &TenantContext,
        thread_id: ThreadId,
    ) -> anyhow::Result<Option<Thread>> {
        self.thread_store
            .get(&tenant.tenant_id, &thread_id.to_string())
            .await
    }
}
