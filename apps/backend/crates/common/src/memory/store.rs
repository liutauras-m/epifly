use super::thread::{Message, Thread};
use super::workspace::{NodeKind, WorkspaceNode};
use async_trait::async_trait;
use thiserror::Error;
use ulid::Ulid;

// ── Typed workspace store error ───────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum WorkspaceStoreError {
    #[error("validation: {0}")]
    Validation(String),
    #[error("node not found")]
    NotFound,
    #[error("forbidden")]
    Forbidden,
    #[error("conflict")]
    Conflict,
    #[error("storage: {0}")]
    Storage(String),
}

impl From<anyhow::Error> for WorkspaceStoreError {
    fn from(e: anyhow::Error) -> Self {
        let msg = e.to_string();
        if msg.contains("not found") {
            Self::NotFound
        } else if msg.contains("validation error") || msg.contains("invalid name") {
            Self::Validation(msg)
        } else if msg.contains("forbidden") || msg.contains("permission denied") {
            Self::Forbidden
        } else if msg.contains("conflict") || msg.contains("already exists") {
            Self::Conflict
        } else {
            Self::Storage(msg)
        }
    }
}

impl From<crate::error::ConusAiError> for WorkspaceStoreError {
    fn from(e: crate::error::ConusAiError) -> Self {
        match e {
            crate::error::ConusAiError::NotFound(_) => Self::NotFound,
            crate::error::ConusAiError::Validation(msg) => Self::Validation(msg),
            _ => Self::Storage(e.to_string()),
        }
    }
}

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
    ) -> Result<WorkspaceNode, WorkspaceStoreError>;

    async fn create_conversation(
        &self,
        tenant_id: &str,
        owner_id: &str,
        parent_id: Option<Ulid>,
        name: &str,
    ) -> Result<WorkspaceNode, WorkspaceStoreError>;

    async fn list_accessible_children(
        &self,
        tenant_id: &str,
        user_id: &str,
        parent_id: Option<Ulid>,
    ) -> Result<Vec<WorkspaceNode>, WorkspaceStoreError>;

    async fn get_accessible_node(
        &self,
        tenant_id: &str,
        user_id: &str,
        id: Ulid,
    ) -> Result<WorkspaceNode, WorkspaceStoreError>;

    async fn get_ancestors(
        &self,
        tenant_id: &str,
        user_id: &str,
        node_id: Ulid,
    ) -> Result<Vec<WorkspaceNode>, WorkspaceStoreError>;

    async fn move_node(
        &self,
        tenant_id: &str,
        user_id: &str,
        node_id: Ulid,
        new_parent: Option<Ulid>,
        new_parent_path: Option<&str>,
    ) -> Result<WorkspaceNode, WorkspaceStoreError>;

    /// Enumerate the root node and all its descendants that would be removed by
    /// `delete_node`. Must be called **before** `delete_node` so that cleanup
    /// logic retains the node list even when the store cascades.
    async fn plan_delete(
        &self,
        tenant_id: &str,
        node_id: Ulid,
    ) -> Result<Vec<DeletePlanNode>, WorkspaceStoreError>;

    async fn delete_node(
        &self,
        tenant_id: &str,
        user_id: &str,
        node_id: Ulid,
    ) -> Result<(), WorkspaceStoreError>;

    /// Rename a workspace node. Protected root folders can be renamed by admins
    /// (`tenant:admin` role is checked at the route layer, not here).
    /// Returns the updated node.
    async fn rename_node(
        &self,
        tenant_id: &str,
        user_id: &str,
        node_id: Ulid,
        new_name: String,
    ) -> Result<WorkspaceNode, WorkspaceStoreError>;

    async fn share_node(
        &self,
        tenant_id: &str,
        owner_id: &str,
        node_id: Ulid,
        with_user_id: &str,
    ) -> Result<WorkspaceNode, WorkspaceStoreError>;

    async fn unshare_node(
        &self,
        tenant_id: &str,
        owner_id: &str,
        node_id: Ulid,
        with_user_id: &str,
    ) -> Result<WorkspaceNode, WorkspaceStoreError>;

    async fn bump_last_modified(
        &self,
        tenant_id: &str,
        node_id: Ulid,
    ) -> Result<(), WorkspaceStoreError>;

    /// Full-text search over node names AND virtual_path accessible to `user_id`.
    async fn search_nodes(
        &self,
        tenant_id: &str,
        user_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<WorkspaceNode>, WorkspaceStoreError>;

    /// Semantic (embedding + ANN) search over content accessible to `user_id`.
    /// Falls back to `search_nodes` if the store does not support embeddings.
    async fn semantic_search_nodes(
        &self,
        tenant_id: &str,
        user_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<WorkspaceNode>, WorkspaceStoreError>;

    /// Store a content snippet and persist its embedding so it can be searched.
    /// Called after each successful RustFS write in `patch_content`.
    /// `content` is chunked and truncated before indexing.
    async fn index_content(
        &self,
        tenant_id: &str,
        node_id: Ulid,
        content: &str,
    ) -> Result<(), WorkspaceStoreError>;

    /// Persist `thread_id` into `metadata.thread_id`. Idempotent; merges into existing
    /// metadata rather than overwriting. Caller is responsible for the access check
    /// (typically already done via `get_accessible_node`).
    async fn bind_thread(
        &self,
        tenant_id: &str,
        node_id: Ulid,
        thread_id: &str,
    ) -> Result<WorkspaceNode, WorkspaceStoreError>;

    /// Returns true if the tenant has been provisioned with a default workspace root folder.
    async fn is_tenant_seeded(&self, tenant_id: &str) -> Result<bool, WorkspaceStoreError>;

    /// Mark the tenant as seeded — idempotent.
    async fn mark_tenant_seeded(&self, tenant_id: &str) -> Result<(), WorkspaceStoreError>;

    /// Create a protected root folder (parent_id = None) that cannot be deleted or moved.
    async fn create_protected_root_folder(
        &self,
        tenant_id: &str,
        owner_id: &str,
        name: &str,
    ) -> Result<WorkspaceNode, WorkspaceStoreError>;

    /// Permanently delete all workspace nodes, threads, messages, audit events, and the
    /// seeding flag for `tenant_id`. Called during tenant teardown — irreversible.
    async fn purge_tenant_data(&self, tenant_id: &str) -> Result<(), WorkspaceStoreError>;

    /// Insert or replace a fully-constructed `WorkspaceNode` with a pre-set id.
    ///
    /// Used by the thread projection system to create/update `Thread`-kind nodes
    /// with a deterministic id derived from `(tenant_id, thread_id)`.
    async fn upsert_node(&self, node: WorkspaceNode) -> Result<(), WorkspaceStoreError>;

    /// Return the `hidden_at` timestamp for a node (if hidden). `None` means visible.
    async fn get_hidden_at(
        &self,
        tenant_id: &str,
        node_id: Ulid,
    ) -> Result<Option<chrono::DateTime<chrono::Utc>>, WorkspaceStoreError>;

    /// Soft-hide a node (set `hidden_at = now`). Used for delete-as-pause on Thread nodes.
    async fn hide_node(&self, tenant_id: &str, node_id: Ulid) -> Result<(), WorkspaceStoreError>;

    /// Un-hide a node (clear `hidden_at`). Used by the projection restore endpoint.
    async fn unhide_node(&self, tenant_id: &str, node_id: Ulid) -> Result<(), WorkspaceStoreError>;

    /// Return all `File`/`Conversation` nodes whose `object_key` is `None` — i.e. nodes
    /// that were created before the Step 3.4 stable-key migration and still use the legacy
    /// `virtual_path` key.
    ///
    /// Used exclusively by `WorkspaceBackfillObjectKeyJob` (Step 3.5). The result is not
    /// ordered; the job processes all entries and marks each one done by setting its
    /// `object_key` via `upsert_node`, so the scan is naturally idempotent and resumable
    /// (already-migrated nodes disappear from the result on the next run).
    ///
    /// Default implementation returns `Ok(vec![])` — safe for stores that do not need the
    /// backfill (e.g., pure in-memory test stores that never persist).
    async fn scan_nodes_needing_backfill(&self) -> Result<Vec<WorkspaceNode>, WorkspaceStoreError> {
        Ok(vec![])
    }
}

/// Reads and writes the markdown body of Conversation nodes from RustFS.
///
/// Step 3.4 migration: every method takes a `key` (primary, used first) and an optional
/// `legacy_key` (only present when `WorkspaceNode.object_key` is `Some`).
///
/// - `key`        = `object_key` when available, otherwise `virtual_path`.
/// - `legacy_key` = `Some(virtual_path)` when `object_key` is set, `None` otherwise.
///
/// Implementations use the pair to perform dual-read (try `key` first, fall back to
/// `legacy_key`) and dual-write (write `key` as primary; mirror to `legacy_key`
/// best-effort). Callers that have no `WorkspaceNode` (e.g. context-builder paths) pass
/// `legacy_key = None`.
#[async_trait]
pub trait WorkspaceContentStore: Send + Sync + 'static {
    /// Returns `""` if the object doesn't exist yet (newly created conversation).
    async fn read(
        &self,
        tenant_id: &str,
        key: &str,
        legacy_key: Option<&str>,
    ) -> anyhow::Result<String>;

    async fn write(
        &self,
        tenant_id: &str,
        key: &str,
        legacy_key: Option<&str>,
        body: &str,
    ) -> anyhow::Result<()>;

    async fn delete(
        &self,
        tenant_id: &str,
        key: &str,
        legacy_key: Option<&str>,
    ) -> anyhow::Result<()>;

    /// Delete the object and all its stored versions (best-effort).
    /// The default implementation delegates to `delete`; versioning-aware
    /// backends override this with a full version sweep.
    async fn delete_all_versions(
        &self,
        tenant_id: &str,
        key: &str,
        legacy_key: Option<&str>,
    ) -> anyhow::Result<()> {
        self.delete(tenant_id, key, legacy_key).await
    }
}
