use super::thread::{Message, Thread};
use super::workspace::{NodeKind, WorkspaceNode};
use async_trait::async_trait;
use ulid::Ulid;

/// Minimal node reference captured before a delete for post-delete cleanup.
#[derive(Debug, Clone)]
pub struct DeletePlanNode {
    pub id: Ulid,
    pub kind: NodeKind,
    pub virtual_path: String,
    /// S3 object key override. When `None`, `virtual_path` is used for content cleanup.
    pub object_key: Option<String>,
}

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
/// markdown body in `WorkspaceContentStore` (RustFS).
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

    /// Enumerate the root node and all its descendants that would be removed by
    /// `delete_node`. Must be called **before** `delete_node` so that cleanup
    /// logic retains the node list even when the store cascades.
    async fn plan_delete(
        &self,
        tenant_id: &str,
        node_id: Ulid,
    ) -> anyhow::Result<Vec<DeletePlanNode>>;

    async fn delete_node(
        &self,
        tenant_id: &str,
        user_id: &str,
        node_id: Ulid,
    ) -> anyhow::Result<()>;

    /// Rename a workspace node. Protected root folders can be renamed by admins
    /// (`tenant:admin` role is checked at the route layer, not here).
    /// Returns the updated node.
    async fn rename_node(
        &self,
        tenant_id: &str,
        user_id: &str,
        node_id: Ulid,
        new_name: String,
    ) -> anyhow::Result<WorkspaceNode>;

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
    /// Called after each successful RustFS write in `patch_content`.
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

    /// Returns true if the tenant has been provisioned with a default workspace root folder.
    async fn is_tenant_seeded(&self, tenant_id: &str) -> anyhow::Result<bool>;

    /// Mark the tenant as seeded — idempotent.
    async fn mark_tenant_seeded(&self, tenant_id: &str) -> anyhow::Result<()>;

    /// Create a protected root folder (parent_id = None) that cannot be deleted or moved.
    async fn create_protected_root_folder(
        &self,
        tenant_id: &str,
        owner_id: &str,
        name: &str,
    ) -> anyhow::Result<WorkspaceNode>;

    /// Permanently delete all workspace nodes, threads, messages, audit events, and the
    /// seeding flag for `tenant_id`. Called during tenant teardown — irreversible.
    async fn purge_tenant_data(&self, tenant_id: &str) -> anyhow::Result<()>;
}

/// Reads and writes the markdown body of Conversation nodes from RustFS.
#[async_trait]
pub trait WorkspaceContentStore: Send + Sync + 'static {
    /// Returns `""` if the object doesn't exist yet (newly created conversation).
    async fn read(&self, tenant_id: &str, virtual_path: &str) -> anyhow::Result<String>;
    async fn write(&self, tenant_id: &str, virtual_path: &str, body: &str) -> anyhow::Result<()>;
    async fn delete(&self, tenant_id: &str, virtual_path: &str) -> anyhow::Result<()>;
    /// Delete the object and all its stored versions (best-effort).
    /// The default implementation delegates to `delete`; versioning-aware
    /// backends override this with a full version sweep.
    async fn delete_all_versions(
        &self,
        tenant_id: &str,
        virtual_path: &str,
    ) -> anyhow::Result<()> {
        self.delete(tenant_id, virtual_path).await
    }
}
