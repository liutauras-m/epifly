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
    NodeKind, WorkspaceNode, WorkspaceNodeKind, effective_user_id, join_virtual_path, validate_name,
};
use common::types::ThreadId;
use redb::{Database, ReadableTable, TableDefinition};
use serde_json::json;
use std::path::Path;
use std::sync::Arc;
use tokio::task;
use tracing::{instrument, warn};
use ulid::Ulid;

// ── Table definitions ─────────────────────────────────────────────────────────

/// threads table: (tenant_id, thread_id) → postcard(Thread)
const THREADS: TableDefinition<(&str, &str), &[u8]> = TableDefinition::new("threads");

/// messages table: (tenant_id, thread_id, seq) → postcard(Message)
const MESSAGES: TableDefinition<(&str, &str, u64), &[u8]> = TableDefinition::new("messages");

/// workspace nodes: (tenant_id, node_id) → json(WorkspaceNode)
const NODES: TableDefinition<(&str, &str), &[u8]> = TableDefinition::new("workspace_nodes");

/// path index: (tenant_id, virtual_path) → node_id
const IDX_PATH: TableDefinition<(&str, &str), &str> = TableDefinition::new("idx_nodes_by_path");

/// audit events: (tenant_id, ts_micros, event_id) → postcard(AuditEvent)
const AUDIT: TableDefinition<(&str, i64, &str), &[u8]> = TableDefinition::new("audit_events");

/// tenant seeding flags: tenant_id → bool (stored as 1-byte)
const TENANT_SEEDED: TableDefinition<&str, u8> = TableDefinition::new("tenant_seeded");

// ── Store ─────────────────────────────────────────────────────────────────────

pub struct RedbMetadataStore {
    db: Arc<Database>,
}

impl RedbMetadataStore {
    pub fn open(path: impl AsRef<Path>) -> anyhow::Result<Arc<Self>> {
        let db = Database::create(path)?;
        Ok(Arc::new(Self::from_db(db)))
    }

    pub fn in_memory() -> anyhow::Result<Arc<Self>> {
        let db = Database::builder().create_with_backend(redb::backends::InMemoryBackend::new())?;
        Ok(Arc::new(Self::from_db(db)))
    }

    fn from_db(db: Database) -> Self {
        let store = Self { db: Arc::new(db) };
        // Ensure all tables exist by opening a write txn on startup.
        if let Ok(txn) = store.db.begin_write() {
            let _ = txn.open_table(THREADS);
            let _ = txn.open_table(MESSAGES);
            let _ = txn.open_table(NODES);
            let _ = txn.open_table(IDX_PATH);
            let _ = txn.open_table(AUDIT);
            let _ = txn.open_table(TENANT_SEEDED);
            let _ = txn.commit();
        }
        store
    }

    /// Return the underlying shared Database handle (used by stores that
    /// must share the same redb file, e.g. CredentialStore).
    pub fn db(&self) -> Arc<Database> {
        Arc::clone(&self.db)
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn ser<T: serde::Serialize>(v: &T) -> anyhow::Result<Vec<u8>> {
    postcard::to_allocvec(v).map_err(|e| anyhow::anyhow!("serialize: {e}"))
}

fn de<'a, T: serde::Deserialize<'a>>(bytes: &'a [u8]) -> anyhow::Result<T> {
    postcard::from_bytes(bytes).map_err(|e| anyhow::anyhow!("deserialize: {e}"))
}

fn ser_node(v: &WorkspaceNode) -> anyhow::Result<Vec<u8>> {
    serde_json::to_vec(v).map_err(|e| anyhow::anyhow!("serialize node: {e}"))
}

fn de_node(bytes: &[u8]) -> anyhow::Result<WorkspaceNode> {
    let mut node: WorkspaceNode =
        serde_json::from_slice(bytes).map_err(|e| anyhow::anyhow!("deserialize node: {e}"))?;
    // Backfill semantic_kind for pre-Step-5.1 folder nodes whose JSON lacked the field.
    if node.kind == NodeKind::Folder && node.semantic_kind == WorkspaceNodeKind::File {
        node.semantic_kind = WorkspaceNodeKind::Folder;
    }
    Ok(node)
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
                (tenant.as_str(), tid.as_str(), 0)..=(tenant.as_str(), tid.as_str(), u64::MAX),
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
                    (tenant.as_str(), tid.as_str(), 0)..=(tenant.as_str(), tid.as_str(), u64::MAX),
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
                let existing = tbl
                    .get((tenant.as_str(), tid.as_str()))?
                    .map(|v| v.value().to_vec());
                if let Some(bytes) = existing {
                    let mut t: Thread = de(&bytes)?;
                    t.last_active = Utc::now();
                    t.message_count += 1;
                    tbl.insert((tenant.as_str(), tid.as_str()), ser(&t)?.as_slice())?;
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
            threads.sort_by_key(|t| std::cmp::Reverse(t.last_active));
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
                let existing = tbl
                    .get((tenant.as_str(), tid.as_str()))?
                    .map(|v| v.value().to_vec());
                if let Some(bytes) = existing {
                    let mut t: Thread = de(&bytes)?;
                    f(&mut t);
                    tbl.insert((tenant.as_str(), tid.as_str()), ser(&t)?.as_slice())?;
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
    ) -> Result<WorkspaceNode, common::memory::store::WorkspaceStoreError> {
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
    ) -> Result<WorkspaceNode, common::memory::store::WorkspaceStoreError> {
        validate_name(name, NodeKind::Conversation)?;
        let parent_path = self.get_node_path(tenant_id, parent_id).await?;
        let virtual_path = join_virtual_path(parent_path.as_deref(), name);
        let node =
            WorkspaceNode::new_conversation(tenant_id, owner_id, parent_id, name, &virtual_path);
        self.insert_node(node.clone()).await?;
        Ok(node)
    }

    async fn list_accessible_children(
        &self,
        tenant_id: &str,
        _user_id: &str,
        parent_id: Option<Ulid>,
    ) -> Result<Vec<WorkspaceNode>, common::memory::store::WorkspaceStoreError> {
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
                let node = match de_node(v.value()) {
                    Ok(node) => node,
                    Err(err) => {
                        warn!(error = %err, "skipping unreadable workspace node row");
                        continue;
                    }
                };
                if node.parent_id == parent_id {
                    nodes.push(node);
                }
            }
            nodes.sort_by(|a, b| a.name.cmp(&b.name));
            Ok(nodes)
        })
        .await
        .unwrap_or_else(|e| Err(anyhow::anyhow!(e)))
        .map_err(Into::into)
    }

    async fn get_accessible_node(
        &self,
        tenant_id: &str,
        _user_id: &str,
        id: Ulid,
    ) -> Result<WorkspaceNode, common::memory::store::WorkspaceStoreError> {
        let tenant = tenant_id.to_string();
        let node_id = id.to_string();
        let db = Arc::clone(&self.db);
        task::spawn_blocking(move || -> anyhow::Result<WorkspaceNode> {
            let txn = db.begin_read()?;
            let tbl = txn.open_table(NODES)?;
            match tbl.get((tenant.as_str(), node_id.as_str()))? {
                Some(v) => de_node(v.value()),
                None => anyhow::bail!("node {node_id} not found"),
            }
        })
        .await
        .unwrap_or_else(|e| Err(anyhow::anyhow!(e)))
        .map_err(Into::into)
    }

    async fn get_ancestors(
        &self,
        tenant_id: &str,
        user_id: &str,
        node_id: Ulid,
    ) -> Result<Vec<WorkspaceNode>, common::memory::store::WorkspaceStoreError> {
        let mut ancestors = Vec::new();
        let mut current = self
            .get_accessible_node(tenant_id, user_id, node_id)
            .await?;
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
    ) -> Result<WorkspaceNode, common::memory::store::WorkspaceStoreError> {
        let mut node = self
            .get_accessible_node(tenant_id, user_id, node_id)
            .await?;
        let parent_path = self.get_node_path(tenant_id, new_parent).await?;
        node.parent_id = new_parent;
        node.virtual_path = join_virtual_path(parent_path.as_deref(), &node.name);
        node.last_modified = Utc::now();
        self.insert_node(node.clone()).await?;
        Ok(node)
    }

    async fn plan_delete(
        &self,
        tenant_id: &str,
        node_id: Ulid,
    ) -> Result<
        Vec<common::memory::store::DeletePlanNode>,
        common::memory::store::WorkspaceStoreError,
    > {
        let tenant = tenant_id.to_string();
        let root_id = node_id;
        let db = Arc::clone(&self.db);
        task::spawn_blocking(
            move || -> anyhow::Result<Vec<common::memory::store::DeletePlanNode>> {
                let txn = db.begin_read()?;
                let tbl = txn.open_table(NODES)?;

                // Collect all nodes for this tenant once into (id, parent_id, kind, virtual_path).
                let mut all_nodes: Vec<(
                    Ulid,
                    Option<Ulid>,
                    common::memory::workspace::NodeKind,
                    String,
                )> = Vec::new();
                let prefix = tenant.as_str();
                let range = tbl.range((prefix, "")..(prefix, "\x7f"))?;
                for item in range {
                    let (_, v) = item?;
                    let node = de_node(v.value())?;
                    all_nodes.push((node.id, node.parent_id, node.kind, node.virtual_path));
                }

                // BFS from root_id.
                let mut result = Vec::new();
                let mut worklist = vec![root_id];
                while let Some(current) = worklist.pop() {
                    // Collect children.
                    for (nid, parent, _, _) in &all_nodes {
                        if *parent == Some(current) {
                            worklist.push(*nid);
                        }
                    }
                    // Add this node to the plan.
                    if let Some((_, _, kind, vp)) =
                        all_nodes.iter().find(|(nid, _, _, _)| *nid == current)
                    {
                        result.push(common::memory::store::DeletePlanNode {
                            id: current,
                            kind: *kind,
                            virtual_path: vp.clone(),
                            object_key: None,
                        });
                    }
                }
                Ok(result)
            },
        )
        .await
        .unwrap_or_else(|e| Err(anyhow::anyhow!(e)))
        .map_err(Into::into)
    }

    async fn delete_node(
        &self,
        tenant_id: &str,
        _user_id: &str,
        node_id: Ulid,
    ) -> Result<(), common::memory::store::WorkspaceStoreError> {
        let tenant = tenant_id.to_string();
        let nid = node_id.to_string();
        let db = Arc::clone(&self.db);
        task::spawn_blocking(move || -> anyhow::Result<()> {
            let txn = db.begin_write()?;
            {
                let mut tbl = txn.open_table(NODES)?;
                // Verify root isn't protected before we start removing anything.
                let root_node = tbl
                    .get((tenant.as_str(), nid.as_str()))?
                    .map(|v| de_node(v.value()))
                    .transpose()?;
                if let Some(ref root) = root_node {
                    if root.is_protected_root {
                        anyhow::bail!("cannot delete protected workspace root folder");
                    }
                } else {
                    return Ok(()); // already gone
                }

                // Collect all tenant node IDs + virtual paths in one scan.
                let all: Vec<(Ulid, Option<Ulid>, String)> = {
                    let prefix = tenant.as_str();
                    let range = tbl.range((prefix, "")..(prefix, "\x7f"))?;
                    let mut v = Vec::new();
                    for item in range {
                        let (_, val) = item?;
                        let n = de_node(val.value())?;
                        v.push((n.id, n.parent_id, n.virtual_path));
                    }
                    v
                };

                // BFS to collect the root and all descendants.
                let mut to_remove: Vec<(Ulid, String)> = Vec::new();
                let mut worklist = vec![ulid::Ulid::from_string(&nid)?];
                while let Some(cur) = worklist.pop() {
                    for (id, parent, _vp) in &all {
                        if *parent == Some(cur) {
                            worklist.push(*id);
                        }
                    }
                    if let Some((_, _, vp)) = all.iter().find(|(id, _, _)| *id == cur) {
                        to_remove.push((cur, vp.clone()));
                    }
                }

                let mut idx = txn.open_table(IDX_PATH)?;
                for (id, vp) in to_remove {
                    let id_str = id.to_string();
                    let _ = tbl.remove((tenant.as_str(), id_str.as_str()))?;
                    let _ = idx.remove((tenant.as_str(), vp.as_str()))?;
                }
            }
            txn.commit()?;
            Ok(())
        })
        .await
        .unwrap_or_else(|e| Err(anyhow::anyhow!(e)))
        .map_err(Into::into)
    }

    async fn rename_node(
        &self,
        tenant_id: &str,
        user_id: &str,
        node_id: Ulid,
        new_name: String,
    ) -> Result<WorkspaceNode, common::memory::store::WorkspaceStoreError> {
        let node = self
            .get_accessible_node(tenant_id, user_id, node_id)
            .await?;
        validate_name(&new_name, node.kind)?;
        let tenant = tenant_id.to_string();
        let nid = node_id.to_string();
        let db = Arc::clone(&self.db);
        let updated = task::spawn_blocking(move || -> anyhow::Result<WorkspaceNode> {
            let txn = db.begin_write()?;
            let updated = {
                let mut tbl = txn.open_table(NODES)?;
                let key = (tenant.as_str(), nid.as_str());
                let raw_bytes = tbl
                    .get(key)?
                    .ok_or_else(|| anyhow::anyhow!("node not found"))?
                    .value()
                    .to_vec();
                let mut n: WorkspaceNode = de_node(&raw_bytes)?;
                let parent_prefix = n
                    .virtual_path
                    .rsplit_once('/')
                    .map(|(p, _)| format!("{p}/"))
                    .unwrap_or_default();
                let old_path = n.virtual_path.clone();
                n.name = new_name.clone();
                n.virtual_path = format!("{parent_prefix}{new_name}");
                n.last_modified = Utc::now();
                tbl.insert(key, ser_node(&n)?.as_slice())?;
                // Update path index.
                let mut idx = txn.open_table(IDX_PATH)?;
                let _ = idx.remove((tenant.as_str(), old_path.as_str()))?;
                idx.insert((tenant.as_str(), n.virtual_path.as_str()), nid.as_str())?;
                n
            };
            txn.commit()?;
            Ok(updated)
        })
        .await
        .unwrap_or_else(|e| Err(anyhow::anyhow!(e)))?;
        Ok(updated)
    }

    async fn share_node(
        &self,
        tenant_id: &str,
        _owner_id: &str,
        node_id: Ulid,
        with_user_id: &str,
    ) -> Result<WorkspaceNode, common::memory::store::WorkspaceStoreError> {
        let uid = with_user_id.to_string();
        self.modify_node(tenant_id, node_id, move |n| {
            if !n.shared_with.contains(&uid) {
                n.shared_with.push(uid);
            }
        })
        .await
        .map_err(Into::into)
    }

    async fn unshare_node(
        &self,
        tenant_id: &str,
        _owner_id: &str,
        node_id: Ulid,
        with_user_id: &str,
    ) -> Result<WorkspaceNode, common::memory::store::WorkspaceStoreError> {
        let uid = with_user_id.to_string();
        self.modify_node(tenant_id, node_id, move |n| {
            n.shared_with.retain(|u| u != &uid);
        })
        .await
        .map_err(Into::into)
    }

    async fn bump_last_modified(
        &self,
        tenant_id: &str,
        node_id: Ulid,
    ) -> Result<(), common::memory::store::WorkspaceStoreError> {
        self.modify_node(tenant_id, node_id, |n| n.last_modified = Utc::now())
            .await
            .map_err(Into::into)
            .map(|_| ())
    }

    async fn search_nodes(
        &self,
        tenant_id: &str,
        _user_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<WorkspaceNode>, common::memory::store::WorkspaceStoreError> {
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
                let node = match de_node(v.value()) {
                    Ok(node) => node,
                    Err(err) => {
                        warn!(error = %err, "skipping unreadable workspace node row during search");
                        continue;
                    }
                };
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
        .await
        .unwrap_or_else(|e| Err(anyhow::anyhow!(e)))
        .map_err(Into::into)
    }

    async fn semantic_search_nodes(
        &self,
        tenant_id: &str,
        user_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<WorkspaceNode>, common::memory::store::WorkspaceStoreError> {
        // Semantic search is done via QdrantVectorStore by the caller.
        // Fall back to name search here.
        self.search_nodes(tenant_id, user_id, query, limit).await
    }

    async fn index_content(
        &self,
        _tenant_id: &str,
        _node_id: Ulid,
        _content: &str,
    ) -> Result<(), common::memory::store::WorkspaceStoreError> {
        // Content indexing is handled by the QdrantVectorStore + EmbeddingService pipeline.
        Ok(())
    }

    async fn bind_thread(
        &self,
        tenant_id: &str,
        node_id: Ulid,
        thread_id: &str,
    ) -> Result<WorkspaceNode, common::memory::store::WorkspaceStoreError> {
        let tid = thread_id.to_string();
        self.modify_node(tenant_id, node_id, move |n| {
            if n.metadata.is_null() {
                n.metadata = json!({});
            }
            n.metadata["thread_id"] = json!(tid);
        })
        .await
        .map_err(Into::into)
    }

    async fn is_tenant_seeded(
        &self,
        tenant_id: &str,
    ) -> Result<bool, common::memory::store::WorkspaceStoreError> {
        let key = tenant_id.to_string();
        let db = Arc::clone(&self.db);
        task::spawn_blocking(move || -> anyhow::Result<bool> {
            let txn = db.begin_read()?;
            let tbl = match txn.open_table(TENANT_SEEDED) {
                Ok(t) => t,
                Err(redb::TableError::TableDoesNotExist(_)) => return Ok(false),
                Err(e) => return Err(anyhow::anyhow!("open tenant_seeded: {e}")),
            };
            Ok(tbl.get(key.as_str())?.is_some())
        })
        .await
        .unwrap_or_else(|e| Err(anyhow::anyhow!(e)))
        .map_err(Into::into)
    }

    async fn mark_tenant_seeded(
        &self,
        tenant_id: &str,
    ) -> Result<(), common::memory::store::WorkspaceStoreError> {
        let key = tenant_id.to_string();
        let db = Arc::clone(&self.db);
        task::spawn_blocking(move || -> anyhow::Result<()> {
            let txn = db.begin_write()?;
            {
                let mut tbl = txn.open_table(TENANT_SEEDED)?;
                tbl.insert(key.as_str(), 1u8)?;
            }
            txn.commit()?;
            Ok(())
        })
        .await
        .unwrap_or_else(|e| Err(anyhow::anyhow!(e)))
        .map_err(Into::into)
    }

    async fn create_protected_root_folder(
        &self,
        tenant_id: &str,
        owner_id: &str,
        name: &str,
    ) -> Result<WorkspaceNode, common::memory::store::WorkspaceStoreError> {
        validate_name(name, NodeKind::Folder)?;
        let virtual_path = name.to_owned();
        let mut node = WorkspaceNode::new_folder(tenant_id, owner_id, None, name, &virtual_path);
        node.is_protected_root = true;
        self.insert_node(node.clone()).await?;
        Ok(node)
    }

    async fn purge_tenant_data(
        &self,
        tenant_id: &str,
    ) -> Result<(), common::memory::store::WorkspaceStoreError> {
        let tenant = tenant_id.to_string();
        let db = Arc::clone(&self.db);
        task::spawn_blocking(move || -> anyhow::Result<()> {
            let txn = db.begin_write()?;
            // Workspace nodes
            {
                let mut tbl = txn.open_table(NODES)?;
                let keys: Vec<(String, String)> = tbl
                    .range((tenant.as_str(), "")..(tenant.as_str(), "\u{FFFF}"))?
                    .map(|r| r.map(|(k, _)| (k.value().0.to_string(), k.value().1.to_string())))
                    .collect::<Result<_, _>>()?;
                for (t, n) in &keys {
                    tbl.remove((t.as_str(), n.as_str()))?;
                }
            }
            // Path index
            {
                let mut idx = txn.open_table(IDX_PATH)?;
                let keys: Vec<(String, String)> = idx
                    .range((tenant.as_str(), "")..(tenant.as_str(), "\u{FFFF}"))?
                    .map(|r| r.map(|(k, _)| (k.value().0.to_string(), k.value().1.to_string())))
                    .collect::<Result<_, _>>()?;
                for (t, p) in &keys {
                    idx.remove((t.as_str(), p.as_str()))?;
                }
            }
            // Threads
            {
                let mut tbl = txn.open_table(THREADS)?;
                let keys: Vec<(String, String)> = tbl
                    .range((tenant.as_str(), "")..(tenant.as_str(), "\u{FFFF}"))?
                    .map(|r| r.map(|(k, _)| (k.value().0.to_string(), k.value().1.to_string())))
                    .collect::<Result<_, _>>()?;
                for (t, tid) in &keys {
                    tbl.remove((t.as_str(), tid.as_str()))?;
                }
            }
            // Messages (triple-key table)
            {
                let mut tbl = txn.open_table(MESSAGES)?;
                let keys: Vec<(String, String, u64)> = tbl
                    .range((tenant.as_str(), "", 0)..(tenant.as_str(), "\u{FFFF}", u64::MAX))?
                    .map(|r| {
                        r.map(|(k, _)| {
                            let (t, tid, seq) = k.value();
                            (t.to_string(), tid.to_string(), seq)
                        })
                    })
                    .collect::<Result<_, _>>()?;
                for (t, tid, seq) in &keys {
                    tbl.remove((t.as_str(), tid.as_str(), *seq))?;
                }
            }
            // Tenant seeding flag
            {
                let mut tbl = txn.open_table(TENANT_SEEDED)?;
                tbl.remove(tenant.as_str())?;
            }
            txn.commit()?;
            Ok(())
        })
        .await
        .unwrap_or_else(|e| Err(anyhow::anyhow!(e)))
        .map_err(Into::into)
    }

    async fn upsert_node(
        &self,
        node: WorkspaceNode,
    ) -> Result<(), common::memory::store::WorkspaceStoreError> {
        self.insert_node(node).await.map_err(Into::into)
    }

    async fn get_hidden_at(
        &self,
        tenant_id: &str,
        node_id: Ulid,
    ) -> Result<Option<chrono::DateTime<chrono::Utc>>, common::memory::store::WorkspaceStoreError>
    {
        let tenant = tenant_id.to_string();
        let nid = node_id.to_string();
        let db = Arc::clone(&self.db);
        task::spawn_blocking(move || {
            let rtx = db.begin_read()?;
            let tbl = rtx.open_table(NODES)?;
            match tbl.get((tenant.as_str(), nid.as_str()))? {
                Some(v) => {
                    let n = de_node(v.value())?;
                    Ok(n.hidden_at)
                }
                None => Ok(None),
            }
        })
        .await
        .unwrap_or_else(|e| Err(anyhow::anyhow!(e)))
        .map_err(Into::into)
    }

    async fn hide_node(
        &self,
        tenant_id: &str,
        node_id: Ulid,
    ) -> Result<(), common::memory::store::WorkspaceStoreError> {
        self.modify_node(tenant_id, node_id, |n| {
            n.hidden_at = Some(chrono::Utc::now());
        })
        .await
        .map(|_| ())
        .map_err(Into::into)
    }

    async fn unhide_node(
        &self,
        tenant_id: &str,
        node_id: Ulid,
    ) -> Result<(), common::memory::store::WorkspaceStoreError> {
        self.modify_node(tenant_id, node_id, |n| {
            n.hidden_at = None;
        })
        .await
        .map(|_| ())
        .map_err(Into::into)
    }

    async fn scan_nodes_needing_backfill(
        &self,
    ) -> Result<Vec<WorkspaceNode>, common::memory::store::WorkspaceStoreError> {
        let db = Arc::clone(&self.db);
        task::spawn_blocking(move || -> anyhow::Result<Vec<WorkspaceNode>> {
            let txn = db.begin_read()?;
            let tbl = txn.open_table(NODES)?;
            let mut result = Vec::new();
            for item in tbl.iter()? {
                let (_, v) = item?;
                match de_node(v.value()) {
                    Ok(node) if node.object_key.is_none() => result.push(node),
                    Ok(_) => {} // already migrated
                    Err(e) => {
                        tracing::warn!(error = %e, "backfill scan: skipping unreadable node row");
                    }
                }
            }
            Ok(result)
        })
        .await
        .unwrap_or_else(|e| Err(anyhow::anyhow!(e)))
        .map_err(Into::into)
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
                tbl.insert((tenant.as_str(), nid.as_str()), ser_node(&node)?.as_slice())?;
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
                let mut node = de_node(v.value())?;
                drop(v);
                f(&mut node);
                tbl.insert((tenant.as_str(), nid.as_str()), ser_node(&node)?.as_slice())?;
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
            let range =
                tbl.range((tenant.as_str(), i64::MIN, "")..(tenant.as_str(), i64::MAX, "\x7f"))?;
            for item in range {
                let (_, v) = item?;
                events.push(de(v.value())?);
            }
            events.sort_by_key(|e| std::cmp::Reverse(e.timestamp));
            events.truncate(limit);
            Ok(events)
        })
        .await
        .map_err(|e| common::error::ConusAiError::Storage(e.to_string()))?
        .map_err(|e: anyhow::Error| common::error::ConusAiError::Storage(e.to_string()))
    }

    async fn prune_before(&self, before: chrono::DateTime<Utc>) -> CResult<u64> {
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
