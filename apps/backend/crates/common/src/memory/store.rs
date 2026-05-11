use super::thread::{Message, Thread};
use super::workspace::WorkspaceNode;
use async_trait::async_trait;
use ulid::Ulid;

/// Pluggable persistent thread store.
//
/// Implementations: `RedbMetadataStore` (agent-core crate) for production;
/// `InMemoryThreadStore` for test mode.
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
    ///
    /// `after` is an optional ULID cursor — returns only threads whose `last_active`
    /// timestamp is strictly before the thread identified by `after`.  Pass `None` for
    /// the first page.
    async fn list(
        &self,
        tenant_id: &str,
        limit: usize,
        after: Option<&str>,
    ) -> anyhow::Result<Vec<Thread>>;

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

/// Persistent workspace node store backed by Postgres.
///
/// Separation of concerns: node metadata + vector embeddings in Postgres;
/// markdown body in `WorkspaceContentStore` (MinIO).
#[async_trait]
pub trait WorkspaceStore: Send + Sync + 'static {
    async fn create_folder(
        &self,
        tenant_id: &str,
        owner_id: &str,
        parent_id: Option<Ulid>,
        name: &str,
    ) -> anyhow::Result<WorkspaceNode>;

    async fn create_conversation(
        &self,
        tenant_id: &str,
        owner_id: &str,
        parent_id: Option<Ulid>,
        name: &str,
    ) -> anyhow::Result<WorkspaceNode>;

    async fn list_accessible_children(
        &self,
        tenant_id: &str,
        user_id: &str,
        parent_id: Option<Ulid>,
    ) -> anyhow::Result<Vec<WorkspaceNode>>;

    async fn get_accessible_node(
        &self,
        tenant_id: &str,
        user_id: &str,
        id: Ulid,
    ) -> anyhow::Result<WorkspaceNode>;

    async fn get_ancestors(
        &self,
        tenant_id: &str,
        user_id: &str,
        node_id: Ulid,
    ) -> anyhow::Result<Vec<WorkspaceNode>>;

    async fn move_node(
        &self,
        tenant_id: &str,
        user_id: &str,
        node_id: Ulid,
        new_parent: Option<Ulid>,
        new_parent_path: Option<&str>,
    ) -> anyhow::Result<WorkspaceNode>;

    async fn delete_node(
        &self,
        tenant_id: &str,
        user_id: &str,
        node_id: Ulid,
    ) -> anyhow::Result<()>;

    async fn share_node(
        &self,
        tenant_id: &str,
        owner_id: &str,
        node_id: Ulid,
        with_user_id: &str,
    ) -> anyhow::Result<WorkspaceNode>;

    async fn unshare_node(
        &self,
        tenant_id: &str,
        owner_id: &str,
        node_id: Ulid,
        with_user_id: &str,
    ) -> anyhow::Result<WorkspaceNode>;

    async fn bump_last_modified(&self, tenant_id: &str, node_id: Ulid) -> anyhow::Result<()>;

    /// Full-text search over node names AND virtual_path accessible to `user_id`.
    async fn search_nodes(
        &self,
        tenant_id: &str,
        user_id: &str,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<WorkspaceNode>>;

    /// Semantic (embedding + ANN) search over content accessible to `user_id`.
    /// Falls back to `search_nodes` if the store does not support embeddings.
    async fn semantic_search_nodes(
        &self,
        tenant_id: &str,
        user_id: &str,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<WorkspaceNode>>;

    /// Store a content snippet and persist its embedding so it can be searched.
    /// Called after each successful MinIO write in `patch_content`.
    /// `content` is chunked and truncated before indexing.
    async fn index_content(
        &self,
        tenant_id: &str,
        node_id: Ulid,
        content: &str,
    ) -> anyhow::Result<()>;

    /// Persist `thread_id` into `metadata.thread_id`. Idempotent; merges into existing
    /// metadata rather than overwriting. Caller is responsible for the access check
    /// (typically already done via `get_accessible_node`).
    async fn bind_thread(
        &self,
        tenant_id: &str,
        node_id: Ulid,
        thread_id: &str,
    ) -> anyhow::Result<WorkspaceNode>;
}

/// Reads and writes the markdown body of Conversation nodes from MinIO.
#[async_trait]
pub trait WorkspaceContentStore: Send + Sync + 'static {
    /// Returns `""` if the object doesn't exist yet (newly created conversation).
    async fn read(&self, tenant_id: &str, virtual_path: &str) -> anyhow::Result<String>;
    async fn write(&self, tenant_id: &str, virtual_path: &str, body: &str) -> anyhow::Result<()>;
    async fn delete(&self, tenant_id: &str, virtual_path: &str) -> anyhow::Result<()>;
}
