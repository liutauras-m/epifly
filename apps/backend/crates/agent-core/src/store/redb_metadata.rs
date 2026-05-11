//! RedbMetadataStore — single embedded key-value store for all metadata.
//!
//! Implements:
//! - `ThreadStore`       (threads + messages tables)
//! - `WorkspaceStore`    (workspace_nodes + indexes)
//! - `AuditStore`        (audit_events table)
//!
//! All mutations use a single `WriteTransaction` committing atomically.
//! Reads use `ReadTransaction` (non-blocking, snapshot-consistent).
//! All values are serialised with `postcard` (faster + smaller than JSON for
//! internal storage; zero-copy reads on hot paths).
//!
//! Constructor: `RedbMetadataStore::open(path)` for production,
//!              `RedbMetadataStore::in_memory()` for tests.

use async_trait::async_trait;
use chrono::Utc;
use common::audit::{AuditEvent, AuditStore};
use common::error::Result as CResult;
use common::memory::store::{ThreadStore, WorkspaceStore};
use common::memory::thread::{Message, Thread};
use common::memory::workspace::{
    NodeKind, WorkspaceNode, effective_user_id, join_virtual_path, validate_name,
};
use common::types::ThreadId;
use redb::{Database, ReadableTable, TableDefinition};
use serde_json::json;
use std::path::Path;
use std::sync::Arc;
use tokio::task;
use tracing::instrument;
use ulid::Ulid;

// ── Table definitions ─────────────────────────────────────────────────────────

/// threads table: (tenant_id, thread_id) → postcard(Thread)
const THREADS: TableDefinition<(&str, &str), &[u8]> =
    TableDefinition::new("threads");

/// messages table: (tenant_id, thread_id, seq) → postcard(Message)
const MESSAGES: TableDefinition<(&str, &str, u64), &[u8]> =
    TableDefinition::new("messages");

/// workspace nodes: (tenant_id, node_id) → postcard(WorkspaceNode)
const NODES: TableDefinition<(&str, &str), &[u8]> =
    TableDefinition::new("workspace_nodes");

/// path index: (tenant_id, virtual_path) → node_id
const IDX_PATH: TableDefinition<(&str, &str), &str> =
    TableDefinition::new("idx_nodes_by_path");

/// audit events: (tenant_id, ts_micros, event_id) → postcard(AuditEvent)
const AUDIT: TableDefinition<(&str, i64, &str), &[u8]> =
    TableDefinition::new("audit_events");

// ── Store ─────────────────────────────────────────────────────────────────────

pub struct RedbMetadataStore {
    db: Arc<Database>,
    /// In-process broadcast for capability-spec hot-reload (replaces PG LISTEN/NOTIFY).
    spec_tx: tokio::sync::broadcast::Sender<(String, String)>,
}

impl RedbMetadataStore {
    pub fn open(path: impl AsRef<Path>) -> anyhow::Result<Arc<Self>> {
        let db = Database::create(path)?;
        Ok(Arc::new(Self::from_db(db)))
    }

    pub fn in_memory() -> anyhow::Result<Arc<Self>> {
        let db = Database::builder()
            .create_with_backend(redb::backends::InMemoryBackend::new())?;
        Ok(Arc::new(Self::from_db(db)))
    }

    fn from_db(db: Database) -> Self {
        let (spec_tx, _) = tokio::sync::broadcast::channel(256);
        let store = Self { db: Arc::new(db), spec_tx };
        // Ensure all tables exist by opening a write txn on startup.
        if let Ok(txn) = store.db.begin_write() {
            let _ = txn.open_table(THREADS);
            let _ = txn.open_table(MESSAGES);
            let _ = txn.open_table(NODES);
            let _ = txn.open_table(IDX_PATH);
            let _ = txn.open_table(AUDIT);
            let _ = txn.commit();
        }
        store
    }

    /// Subscribe to capability-spec change events (namespace, tool_name).
    /// Replaces PG LISTEN/NOTIFY for hot-reload.
    pub fn subscribe_spec_changes(
        &self,
    ) -> tokio::sync::broadcast::Receiver<(String, String)> {
        self.spec_tx.subscribe()
    }

    pub fn notify_spec_change(&self, namespace: &str, tool_name: &str) {
        let _ = self.spec_tx.send((namespace.to_string(), tool_name.to_string()));
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn ser<T: serde::Serialize>(v: &T) -> anyhow::Result<Vec<u8>> {
    postcard::to_allocvec(v).map_err(|e| anyhow::anyhow!("serialize: {e}"))
}

fn de<'a, T: serde::Deserialize<'a>>(bytes: &'a [u8]) -> anyhow::Result<T> {
    postcard::from_bytes(bytes).map_err(|e| anyhow::anyhow!("deserialize: {e}"))
}

// ── ThreadStore ───────────────────────────────────────────────────────────────

#[async_trait]
impl ThreadStore for RedbMetadataStore {
    async fn create(
        &self,
        tenant_id: &str,
        initial_messages: Vec<Message>,
    ) -> anyhow::Result<Thread> {
        let id = ThreadId::new();
        let thread_id = id.to_string();
        let tenant = tenant_id.to_string();
        let now = Utc::now();
        let thread = Thread {
            id,
            tenant_id: tenant.clone(),
            title: None,
            created_at: now,
            last_active: now,
            message_count: initial_messages.len(),
            summary: None,
            metadata: json!({}),
        };
        let thread_clone = thread.clone();
        let db = Arc::clone(&self.db);
        let msgs = initial_messages;

        task::spawn_blocking(move || -> anyhow::Result<()> {
            let txn = db.begin_write()?;
            {
                let mut tbl = txn.open_table(THREADS)?;
                tbl.insert(
                    (tenant.as_str(), thread_id.as_str()),
                    ser(&thread_clone)?.as_slice(),
                )?;
            }
            {
                let mut tbl = txn.open_table(MESSAGES)?;
                for (i, msg) in msgs.iter().enumerate() {
                    tbl.insert(
                        (tenant.as_str(), thread_id.as_str(), i as u64),
                        ser(msg)?.as_slice(),
                    )?;
                }
            }
            txn.commit()?;
            Ok(())
        })
        .await??;

        Ok(thread)
    }

    async fn get(&self, tenant_id: &str, thread_id: &str) -> anyhow::Result<Option<Thread>> {
        let tenant = tenant_id.to_string();
        let tid = thread_id.to_string();
        let db = Arc::clone(&self.db);
        task::spawn_blocking(move || -> anyhow::Result<Option<Thread>> {
            let txn = db.begin_read()?;
            let tbl = txn.open_table(THREADS)?;
            match tbl.get((tenant.as_str(), tid.as_str()))? {
                Some(v) => Ok(Some(de(v.value())?)),
                None => Ok(None),
            }
        })
        .await?
    }

    async fn messages(&self, tenant_id: &str, thread_id: &str) -> anyhow::Result<Vec<Message>> {
        let tenant = tenant_id.to_string();
        let tid = thread_id.to_string();
        let db = Arc::clone(&self.db);
        task::spawn_blocking(move || -> anyhow::Result<Vec<Message>> {
            let txn = db.begin_read()?;
            let tbl = txn.open_table(MESSAGES)?;
            let mut msgs = Vec::new();
            let range = tbl.range(
                (tenant.as_str(), tid.as_str(), 0)
                    ..=(tenant.as_str(), tid.as_str(), u64::MAX),
            )?;
            for item in range {
                let (_, v) = item?;
                msgs.push(de(v.value())?);
            }
            Ok(msgs)
        })
        .await?
    }

    async fn append(
        &self,
        tenant_id: &str,
        thread_id: &str,
        message: Message,
    ) -> anyhow::Result<()> {
        let tenant = tenant_id.to_string();
        let tid = thread_id.to_string();
        let db = Arc::clone(&self.db);
        task::spawn_blocking(move || -> anyhow::Result<()> {
            let txn = db.begin_write()?;
            let seq = {
                let tbl = txn.open_table(MESSAGES)?;
                let mut count = 0u64;
                if let Ok(range) = tbl.range(
                    (tenant.as_str(), tid.as_str(), 0)
                        ..=(tenant.as_str(), tid.as_str(), u64::MAX),
                ) {
                    count = range.count() as u64;
                }
                count
            };
            {
                let mut tbl = txn.open_table(MESSAGES)?;
                tbl.insert(
                    (tenant.as_str(), tid.as_str(), seq),
                    ser(&message)?.as_slice(),
                )?;
            }
            // Bump thread metadata.
            {
                let mut tbl = txn.open_table(THREADS)?;
                let existing = tbl.get((tenant.as_str(), tid.as_str()))?.map(|v| v.value().to_vec());
                if let Some(bytes) = existing {
                    let mut t: Thread = de(&bytes)?;
                    t.last_active = Utc::now();
                    t.message_count += 1;
                    tbl.insert(
                        (tenant.as_str(), tid.as_str()),
                        ser(&t)?.as_slice(),
                    )?;
                }
            }
            txn.commit()?;
            Ok(())
        })
        .await?
    }

    async fn list(
        &self,
        tenant_id: &str,
        limit: usize,
        _after: Option<&str>,
    ) -> anyhow::Result<Vec<Thread>> {
        let tenant = tenant_id.to_string();
        let db = Arc::clone(&self.db);
        task::spawn_blocking(move || -> anyhow::Result<Vec<Thread>> {
            let txn = db.begin_read()?;
            let tbl = txn.open_table(THREADS)?;
            let mut threads = Vec::new();
            let prefix = tenant.as_str();
            let range = tbl.range((prefix, "")..(prefix, "\x7f"))?;
            for item in range {
                let (_, v) = item?;
                threads.push(de::<Thread>(v.value())?);
            }
            threads.sort_by(|a, b| b.last_active.cmp(&a.last_active));
            threads.truncate(limit);
            Ok(threads)
        })
        .await?
    }

    async fn set_summary(
        &self,
        tenant_id: &str,
        thread_id: &str,
        summary: String,
    ) -> anyhow::Result<()> {
        self.update_thread(tenant_id, thread_id, |t| t.summary = Some(summary))
            .await
    }

    async fn set_title(
        &self,
        tenant_id: &str,
        thread_id: &str,
        title: String,
    ) -> anyhow::Result<()> {
        self.update_thread(tenant_id, thread_id, |t| t.title = Some(title))
            .await
    }
}

impl RedbMetadataStore {
    async fn update_thread(
        &self,
        tenant_id: &str,
        thread_id: &str,
        f: impl FnOnce(&mut Thread) + Send + 'static,
    ) -> anyhow::Result<()> {
        let tenant = tenant_id.to_string();
        let tid = thread_id.to_string();
        let db = Arc::clone(&self.db);
        task::spawn_blocking(move || -> anyhow::Result<()> {
            let txn = db.begin_write()?;
            {
                let mut tbl = txn.open_table(THREADS)?;
                let existing = tbl.get((tenant.as_str(), tid.as_str()))?.map(|v| v.value().to_vec());
                if let Some(bytes) = existing {
                    let mut t: Thread = de(&bytes)?;
                    f(&mut t);
                    tbl.insert(
                        (tenant.as_str(), tid.as_str()),
                        ser(&t)?.as_slice(),
                    )?;
                }
            }
            txn.commit()?;
            Ok(())
        })
        .await?
    }
}

// ── WorkspaceStore ────────────────────────────────────────────────────────────

#[async_trait]
impl WorkspaceStore for RedbMetadataStore {
    async fn create_folder(
        &self,
        tenant_id: &str,
        owner_id: &str,
        parent_id: Option<Ulid>,
        name: &str,
    ) -> anyhow::Result<WorkspaceNode> {
        validate_name(name, NodeKind::Folder)?;
        let parent_path = self.get_node_path(tenant_id, parent_id).await?;
        let virtual_path = join_virtual_path(parent_path.as_deref(), name);
        let node = WorkspaceNode::new_folder(tenant_id, owner_id, parent_id, name, &virtual_path);
        self.insert_node(node.clone()).await?;
        Ok(node)
    }

    async fn create_conversation(
        &self,
        tenant_id: &str,
        owner_id: &str,
        parent_id: Option<Ulid>,
        name: &str,
    ) -> anyhow::Result<WorkspaceNode> {
        validate_name(name, NodeKind::Conversation)?;
        let parent_path = self.get_node_path(tenant_id, parent_id).await?;
        let virtual_path = join_virtual_path(parent_path.as_deref(), name);
        let node = WorkspaceNode::new_conversation(tenant_id, owner_id, parent_id, name, &virtual_path);
        self.insert_node(node.clone()).await?;
        Ok(node)
    }

    async fn list_accessible_children(
        &self,
        tenant_id: &str,
        _user_id: &str,
        parent_id: Option<Ulid>,
    ) -> anyhow::Result<Vec<WorkspaceNode>> {
        let tenant = tenant_id.to_string();
        let db = Arc::clone(&self.db);
        task::spawn_blocking(move || -> anyhow::Result<Vec<WorkspaceNode>> {
            let txn = db.begin_read()?;
            let tbl = txn.open_table(NODES)?;
            let mut nodes = Vec::new();
            let prefix = tenant.as_str();
            let range = tbl.range((prefix, "")..(prefix, "\x7f"))?;
            for item in range {
                let (_, v) = item?;
                let node: WorkspaceNode = de(v.value())?;
                if node.parent_id == parent_id {
                    nodes.push(node);
                }
            }
            nodes.sort_by(|a, b| a.name.cmp(&b.name));
            Ok(nodes)
        })
        .await?
    }

    async fn get_accessible_node(
        &self,
        tenant_id: &str,
        _user_id: &str,
        id: Ulid,
    ) -> anyhow::Result<WorkspaceNode> {
        let tenant = tenant_id.to_string();
        let node_id = id.to_string();
        let db = Arc::clone(&self.db);
        task::spawn_blocking(move || -> anyhow::Result<WorkspaceNode> {
            let txn = db.begin_read()?;
            let tbl = txn.open_table(NODES)?;
            match tbl.get((tenant.as_str(), node_id.as_str()))? {
                Some(v) => de(v.value()),
                None => anyhow::bail!("node {node_id} not found"),
            }
        })
        .await?
    }

    async fn get_ancestors(
        &self,
        tenant_id: &str,
        user_id: &str,
        node_id: Ulid,
    ) -> anyhow::Result<Vec<WorkspaceNode>> {
        let mut ancestors = Vec::new();
        let mut current = self.get_accessible_node(tenant_id, user_id, node_id).await?;
        while let Some(pid) = current.parent_id {
            current = self.get_accessible_node(tenant_id, user_id, pid).await?;
            ancestors.insert(0, current.clone());
        }
        Ok(ancestors)
    }

    async fn move_node(
        &self,
        tenant_id: &str,
        user_id: &str,
        node_id: Ulid,
        new_parent: Option<Ulid>,
        _new_parent_path: Option<&str>,
    ) -> anyhow::Result<WorkspaceNode> {
        let mut node = self.get_accessible_node(tenant_id, user_id, node_id).await?;
        let parent_path = self.get_node_path(tenant_id, new_parent).await?;
        node.parent_id = new_parent;
        node.virtual_path = join_virtual_path(parent_path.as_deref(), &node.name);
        node.last_modified = Utc::now();
        self.insert_node(node.clone()).await?;
        Ok(node)
    }

    async fn delete_node(
        &self,
        tenant_id: &str,
        _user_id: &str,
        node_id: Ulid,
    ) -> anyhow::Result<()> {
        let tenant = tenant_id.to_string();
        let nid = node_id.to_string();
        let db = Arc::clone(&self.db);
        task::spawn_blocking(move || -> anyhow::Result<()> {
            let txn = db.begin_write()?;
            {
                let mut tbl = txn.open_table(NODES)?;
                if let Some(v) = tbl.remove((tenant.as_str(), nid.as_str()))? {
                    let node: WorkspaceNode = de(v.value())?;
                    drop(v);
                    let mut idx = txn.open_table(IDX_PATH)?;
                    let _ = idx.remove((tenant.as_str(), node.virtual_path.as_str()))?;
                }
            }
            txn.commit()?;
            Ok(())
        })
        .await?
    }

    async fn share_node(
        &self,
        tenant_id: &str,
        _owner_id: &str,
        node_id: Ulid,
        with_user_id: &str,
    ) -> anyhow::Result<WorkspaceNode> {
        let uid = with_user_id.to_string();
        self.modify_node(tenant_id, node_id, move |n| {
            if !n.shared_with.contains(&uid) {
                n.shared_with.push(uid);
            }
        })
        .await
    }

    async fn unshare_node(
        &self,
        tenant_id: &str,
        _owner_id: &str,
        node_id: Ulid,
        with_user_id: &str,
    ) -> anyhow::Result<WorkspaceNode> {
        let uid = with_user_id.to_string();
        self.modify_node(tenant_id, node_id, move |n| {
            n.shared_with.retain(|u| u != &uid);
        })
        .await
    }

    async fn bump_last_modified(&self, tenant_id: &str, node_id: Ulid) -> anyhow::Result<()> {
        self.modify_node(tenant_id, node_id, |n| n.last_modified = Utc::now())
            .await
            .map(|_| ())
    }

    async fn search_nodes(
        &self,
        tenant_id: &str,
        _user_id: &str,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<WorkspaceNode>> {
        let tenant = tenant_id.to_string();
        let q = query.to_lowercase();
        let db = Arc::clone(&self.db);
        task::spawn_blocking(move || -> anyhow::Result<Vec<WorkspaceNode>> {
            let txn = db.begin_read()?;
            let tbl = txn.open_table(NODES)?;
            let mut nodes = Vec::new();
            let range = tbl.range((tenant.as_str(), "")..(tenant.as_str(), "\x7f"))?;
            for item in range {
                let (_, v) = item?;
                let node: WorkspaceNode = de(v.value())?;
                if node.name.to_lowercase().contains(&q)
                    || node.virtual_path.to_lowercase().contains(&q)
                {
                    nodes.push(node);
                }
                if nodes.len() >= limit {
                    break;
                }
            }
            Ok(nodes)
        })
        .await?
    }

    async fn semantic_search_nodes(
        &self,
        tenant_id: &str,
        user_id: &str,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<WorkspaceNode>> {
        // Semantic search is done via QdrantVectorStore by the caller.
        // Fall back to name search here.
        self.search_nodes(tenant_id, user_id, query, limit).await
    }

    async fn index_content(
        &self,
        _tenant_id: &str,
        _node_id: Ulid,
        _content: &str,
    ) -> anyhow::Result<()> {
        // Content indexing is handled by the QdrantVectorStore + EmbeddingService pipeline.
        Ok(())
    }

    async fn bind_thread(
        &self,
        tenant_id: &str,
        node_id: Ulid,
        thread_id: &str,
    ) -> anyhow::Result<WorkspaceNode> {
        let tid = thread_id.to_string();
        self.modify_node(tenant_id, node_id, move |n| {
            if n.metadata.is_null() {
                n.metadata = json!({});
            }
            n.metadata["thread_id"] = json!(tid);
        })
        .await
    }
}

impl RedbMetadataStore {
    async fn insert_node(&self, node: WorkspaceNode) -> anyhow::Result<()> {
        let tenant = node.tenant_id.clone();
        let nid = node.id.to_string();
        let path = node.virtual_path.clone();
        let db = Arc::clone(&self.db);
        task::spawn_blocking(move || -> anyhow::Result<()> {
            let txn = db.begin_write()?;
            {
                let mut tbl = txn.open_table(NODES)?;
                tbl.insert((tenant.as_str(), nid.as_str()), ser(&node)?.as_slice())?;
            }
            {
                let mut idx = txn.open_table(IDX_PATH)?;
                idx.insert((tenant.as_str(), path.as_str()), nid.as_str())?;
            }
            txn.commit()?;
            Ok(())
        })
        .await?
    }

    async fn modify_node(
        &self,
        tenant_id: &str,
        node_id: Ulid,
        f: impl FnOnce(&mut WorkspaceNode) + Send + 'static,
    ) -> anyhow::Result<WorkspaceNode> {
        let tenant = tenant_id.to_string();
        let nid = node_id.to_string();
        let db = Arc::clone(&self.db);
        task::spawn_blocking(move || -> anyhow::Result<WorkspaceNode> {
            let txn = db.begin_write()?;
            let node = {
                let mut tbl = txn.open_table(NODES)?;
                let v = tbl
                    .get((tenant.as_str(), nid.as_str()))?
                    .ok_or_else(|| anyhow::anyhow!("node {nid} not found"))?;
                let mut node: WorkspaceNode = de(v.value())?;
                drop(v);
                f(&mut node);
                tbl.insert((tenant.as_str(), nid.as_str()), ser(&node)?.as_slice())?;
                node
            };
            txn.commit()?;
            Ok(node)
        })
        .await?
    }

    async fn get_node_path(
        &self,
        tenant_id: &str,
        node_id: Option<Ulid>,
    ) -> anyhow::Result<Option<String>> {
        let Some(id) = node_id else { return Ok(None) };
        let node = self
            .get_accessible_node(tenant_id, effective_user_id(None), id)
            .await?;
        Ok(Some(node.virtual_path))
    }
}

// ── AuditStore ────────────────────────────────────────────────────────────────

#[async_trait]
impl AuditStore for RedbMetadataStore {
    #[instrument(skip(self, event))]
    async fn append(&self, event: AuditEvent) -> CResult<()> {
        let tenant = event.tenant_id.clone();
        let ts = event.timestamp.timestamp_micros();
        let id = event.id.clone();
        let db = Arc::clone(&self.db);
        task::spawn_blocking(move || -> anyhow::Result<()> {
            let txn = db.begin_write()?;
            {
                let mut tbl = txn.open_table(AUDIT)?;
                tbl.insert((tenant.as_str(), ts, id.as_str()), ser(&event)?.as_slice())?;
            }
            txn.commit()?;
            Ok(())
        })
        .await
        .map_err(|e| common::error::ConusAiError::Storage(e.to_string()))?
        .map_err(|e: anyhow::Error| common::error::ConusAiError::Storage(e.to_string()))
    }

    async fn list(
        &self,
        tenant_id: &str,
        limit: usize,
        _after: Option<&str>,
    ) -> CResult<Vec<AuditEvent>> {
        let tenant = tenant_id.to_string();
        let db = Arc::clone(&self.db);
        task::spawn_blocking(move || -> anyhow::Result<Vec<AuditEvent>> {
            let txn = db.begin_read()?;
            let tbl = txn.open_table(AUDIT)?;
            let mut events: Vec<AuditEvent> = Vec::new();
            let range = tbl.range(
                (tenant.as_str(), i64::MIN, "")..(tenant.as_str(), i64::MAX, "\x7f"),
            )?;
            for item in range {
                let (_, v) = item?;
                events.push(de(v.value())?);
            }
            events.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
            events.truncate(limit);
            Ok(events)
        })
        .await
        .map_err(|e| common::error::ConusAiError::Storage(e.to_string()))?
        .map_err(|e: anyhow::Error| common::error::ConusAiError::Storage(e.to_string()))
    }

    async fn prune_before(
        &self,
        before: chrono::DateTime<Utc>,
    ) -> CResult<u64> {
        let cutoff = before.timestamp_micros();
        let db = Arc::clone(&self.db);
        task::spawn_blocking(move || -> anyhow::Result<u64> {
            let txn = db.begin_write()?;
            let mut deleted = 0u64;
            {
                let mut tbl = txn.open_table(AUDIT)?;
                let to_delete: Vec<(String, i64, String)> = tbl
                    .iter()?
                    .filter_map(|r| -> Option<(String, i64, String)> {
                        let (k, _) = r.ok()?;
                        let (t, ts, id) = k.value();
                        if ts < cutoff {
                            Some((t.to_string(), ts, id.to_string()))
                        } else {
                            None
                        }
                    })
                    .collect();
                for (t, ts, id) in to_delete {
                    tbl.remove((t.as_str(), ts, id.as_str()))?;
                    deleted += 1;
                }
            }
            txn.commit()?;
            Ok(deleted)
        })
        .await
        .map_err(|e| common::error::ConusAiError::Storage(e.to_string()))?
        .map_err(|e: anyhow::Error| common::error::ConusAiError::Storage(e.to_string()))
    }
}
