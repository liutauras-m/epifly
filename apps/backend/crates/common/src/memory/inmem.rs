//! In-memory implementations of all store traits.
//!
//! Intended for tests and for local dev when `CONUSAI_TEST_MODE=1` is set.
//! No Postgres or RustFS required — everything lives in locked HashMaps / Vecs.
//!
//! These are **not** thread-safe across process restarts; data is lost on exit.
//! That is intentional — they exist to remove the Docker dependency from tests.
use crate::audit::{AuditEvent, AuditStore};
use crate::memory::store::{ThreadStore, WorkspaceContentStore, WorkspaceStore};
use crate::memory::thread::{Message, Thread};
use crate::memory::workspace::{NodeKind, WorkspaceNode, join_virtual_path, validate_name};
use crate::types::ThreadId;
use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;
use std::cmp::Reverse;
use std::collections::HashMap;
use std::sync::Mutex;
use ulid::Ulid;

// ─── ThreadStore ──────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct InMemoryThreadStore {
    threads: Mutex<HashMap<(String, String), Thread>>,
    messages: Mutex<HashMap<(String, String), Vec<Message>>>,
}

impl InMemoryThreadStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl ThreadStore for InMemoryThreadStore {
    async fn create(
        &self,
        tenant_id: &str,
        initial_messages: Vec<Message>,
    ) -> anyhow::Result<Thread> {
        let id = ThreadId::new();
        let thread_id_str = id.to_string();
        let now = Utc::now();
        let thread = Thread {
            id,
            tenant_id: tenant_id.to_owned(),
            title: None,
            created_at: now,
            last_active: now,
            message_count: initial_messages.len(),
            summary: None,
            metadata: json!({}),
        };
        {
            let mut threads = self.threads.lock().unwrap();
            threads.insert(
                (tenant_id.to_owned(), thread_id_str.clone()),
                thread.clone(),
            );
        }
        {
            let mut msgs = self.messages.lock().unwrap();
            msgs.insert(
                (tenant_id.to_owned(), thread_id_str.clone()),
                initial_messages,
            );
        }
        Ok(thread)
    }

    async fn get(&self, tenant_id: &str, thread_id: &str) -> anyhow::Result<Option<Thread>> {
        let threads = self.threads.lock().unwrap();
        Ok(threads
            .get(&(tenant_id.to_owned(), thread_id.to_owned()))
            .cloned())
    }

    async fn messages(&self, tenant_id: &str, thread_id: &str) -> anyhow::Result<Vec<Message>> {
        let msgs = self.messages.lock().unwrap();
        Ok(msgs
            .get(&(tenant_id.to_owned(), thread_id.to_owned()))
            .cloned()
            .unwrap_or_default())
    }

    async fn append(
        &self,
        tenant_id: &str,
        thread_id: &str,
        message: Message,
    ) -> anyhow::Result<()> {
        let key = (tenant_id.to_owned(), thread_id.to_owned());
        {
            let mut msgs = self.messages.lock().unwrap();
            msgs.entry(key.clone()).or_default().push(message);
        }
        let mut threads = self.threads.lock().unwrap();
        if let Some(t) = threads.get_mut(&key) {
            t.message_count += 1;
            t.last_active = Utc::now();
        }
        Ok(())
    }

    async fn list(
        &self,
        tenant_id: &str,
        limit: usize,
        after: Option<&str>,
    ) -> anyhow::Result<Vec<Thread>> {
        let threads = self.threads.lock().unwrap();
        let mut result: Vec<Thread> = threads
            .iter()
            .filter(|((tid, _), _)| tid == tenant_id)
            .map(|(_, t)| t.clone())
            .collect();
        result.sort_by_key(|t| Reverse(t.last_active));

        // Apply cursor: find the pivot thread and drop everything up to and including it
        if let Some(cursor) = after
            && let Some(pos) = result.iter().position(|t| t.id.to_string() == cursor)
        {
            result = result.into_iter().skip(pos + 1).collect();
        }

        result.truncate(limit);
        Ok(result)
    }

    async fn set_summary(
        &self,
        tenant_id: &str,
        thread_id: &str,
        summary: String,
    ) -> anyhow::Result<()> {
        let mut threads = self.threads.lock().unwrap();
        if let Some(t) = threads.get_mut(&(tenant_id.to_owned(), thread_id.to_owned())) {
            t.summary = Some(summary);
        }
        Ok(())
    }

    async fn set_title(
        &self,
        tenant_id: &str,
        thread_id: &str,
        title: String,
    ) -> anyhow::Result<()> {
        let mut threads = self.threads.lock().unwrap();
        if let Some(t) = threads.get_mut(&(tenant_id.to_owned(), thread_id.to_owned())) {
            t.title = Some(title);
        }
        Ok(())
    }
}

// ─── WorkspaceStore ───────────────────────────────────────────────────────────

#[derive(Default)]
pub struct InMemoryWorkspaceStore {
    nodes: Mutex<HashMap<Ulid, WorkspaceNode>>,
    content: Mutex<HashMap<Ulid, String>>,
    seeded: Mutex<std::collections::HashSet<String>>,
}

impl InMemoryWorkspaceStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn get_node(&self, id: Ulid) -> Option<WorkspaceNode> {
        self.nodes.lock().unwrap().get(&id).cloned()
    }

    fn check_access(node: &WorkspaceNode, user_id: &str) -> bool {
        node.owner_id == user_id || node.shared_with.iter().any(|u| u == user_id)
    }
}

#[async_trait]
impl WorkspaceStore for InMemoryWorkspaceStore {
    async fn create_folder(
        &self,
        tenant_id: &str,
        owner_id: &str,
        parent_id: Option<Ulid>,
        name: &str,
    ) -> anyhow::Result<WorkspaceNode> {
        validate_name(name, NodeKind::Folder).map_err(|e| anyhow::anyhow!("{e}"))?;
        let parent_path = if let Some(pid) = parent_id {
            self.get_node(pid)
                .map(|p| p.virtual_path)
                .ok_or_else(|| anyhow::anyhow!("parent not found"))?
                .into()
        } else {
            None
        };
        let virtual_path = join_virtual_path(parent_path.as_deref(), name);
        let node = WorkspaceNode::new_folder(tenant_id, owner_id, parent_id, name, virtual_path);
        self.nodes.lock().unwrap().insert(node.id, node.clone());
        Ok(node)
    }

    async fn create_conversation(
        &self,
        tenant_id: &str,
        owner_id: &str,
        parent_id: Option<Ulid>,
        name: &str,
    ) -> anyhow::Result<WorkspaceNode> {
        validate_name(name, NodeKind::Conversation).map_err(|e| anyhow::anyhow!("{e}"))?;
        let parent_path = if let Some(pid) = parent_id {
            self.get_node(pid)
                .map(|p| p.virtual_path)
                .ok_or_else(|| anyhow::anyhow!("parent not found"))?
                .into()
        } else {
            None
        };
        let virtual_path = join_virtual_path(parent_path.as_deref(), name);
        let node =
            WorkspaceNode::new_conversation(tenant_id, owner_id, parent_id, name, &virtual_path);
        self.nodes.lock().unwrap().insert(node.id, node.clone());
        Ok(node)
    }

    async fn list_accessible_children(
        &self,
        tenant_id: &str,
        user_id: &str,
        parent_id: Option<Ulid>,
    ) -> anyhow::Result<Vec<WorkspaceNode>> {
        let nodes = self.nodes.lock().unwrap();
        let mut result: Vec<WorkspaceNode> = nodes
            .values()
            .filter(|n| {
                n.tenant_id == tenant_id
                    && n.parent_id == parent_id
                    && Self::check_access(n, user_id)
            })
            .cloned()
            .collect();
        result.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(result)
    }

    async fn get_accessible_node(
        &self,
        tenant_id: &str,
        user_id: &str,
        id: Ulid,
    ) -> anyhow::Result<WorkspaceNode> {
        let node = self
            .get_node(id)
            .filter(|n| n.tenant_id == tenant_id)
            .ok_or_else(|| {
                anyhow::anyhow!(crate::error::ConusAiError::NotFound(format!("node {id}")))
            })?;
        if !Self::check_access(&node, user_id) {
            anyhow::bail!(crate::error::ConusAiError::NotFound(format!("node {id}")));
        }
        Ok(node)
    }

    async fn get_ancestors(
        &self,
        tenant_id: &str,
        user_id: &str,
        node_id: Ulid,
    ) -> anyhow::Result<Vec<WorkspaceNode>> {
        let mut ancestors = vec![];
        let mut current_id = node_id;
        loop {
            let node = match self.get_node(current_id) {
                Some(n) if n.tenant_id == tenant_id => n,
                _ => break,
            };
            if !Self::check_access(&node, user_id) {
                break;
            }
            match node.parent_id {
                Some(pid) => {
                    ancestors.insert(0, node);
                    current_id = pid;
                }
                None => {
                    ancestors.insert(0, node);
                    break;
                }
            }
        }
        Ok(ancestors)
    }

    async fn move_node(
        &self,
        tenant_id: &str,
        user_id: &str,
        node_id: Ulid,
        new_parent: Option<Ulid>,
        new_parent_path: Option<&str>,
    ) -> anyhow::Result<WorkspaceNode> {
        let mut nodes = self.nodes.lock().unwrap();
        let node = nodes
            .get_mut(&node_id)
            .filter(|n| n.tenant_id == tenant_id && Self::check_access(n, user_id))
            .ok_or_else(|| {
                anyhow::anyhow!(crate::error::ConusAiError::NotFound(format!(
                    "node {node_id}"
                )))
            })?;
        node.parent_id = new_parent;
        node.virtual_path = join_virtual_path(new_parent_path, &node.name);
        node.last_modified = Utc::now();
        Ok(node.clone())
    }

    async fn delete_node(
        &self,
        tenant_id: &str,
        user_id: &str,
        node_id: Ulid,
    ) -> anyhow::Result<()> {
        // Verify access first; block deletion of protected roots.
        {
            let nodes = self.nodes.lock().unwrap();
            let node = nodes
                .get(&node_id)
                .filter(|n| n.tenant_id == tenant_id && Self::check_access(n, user_id))
                .ok_or_else(|| {
                    anyhow::anyhow!(crate::error::ConusAiError::NotFound(format!(
                        "node {node_id}"
                    )))
                })?;
            if node.is_protected_root {
                anyhow::bail!("cannot delete protected workspace root folder");
            }
        }

        // Worklist-based recursive delete
        let mut worklist = vec![node_id];
        while let Some(current) = worklist.pop() {
            let children: Vec<Ulid> = {
                let nodes = self.nodes.lock().unwrap();
                nodes
                    .values()
                    .filter(|n| n.tenant_id == tenant_id && n.parent_id == Some(current))
                    .map(|n| n.id)
                    .collect()
            };
            worklist.extend(children);
            self.nodes.lock().unwrap().remove(&current);
            self.content.lock().unwrap().remove(&current);
        }
        Ok(())
    }

    async fn rename_node(
        &self,
        tenant_id: &str,
        user_id: &str,
        node_id: Ulid,
        new_name: String,
    ) -> anyhow::Result<WorkspaceNode> {
        let mut nodes = self.nodes.lock().unwrap();
        let node = nodes
            .get_mut(&node_id)
            .filter(|n| n.tenant_id == tenant_id && Self::check_access(n, user_id))
            .ok_or_else(|| {
                anyhow::anyhow!(crate::error::ConusAiError::NotFound(format!(
                    "node {node_id}"
                )))
            })?;
        validate_name(&new_name, node.kind).map_err(|e| anyhow::anyhow!("{e}"))?;
        let parent_prefix = node
            .virtual_path
            .rsplit_once('/')
            .map(|(p, _)| format!("{p}/"))
            .unwrap_or_default();
        node.name = new_name.clone();
        node.virtual_path = format!("{parent_prefix}{new_name}");
        node.last_modified = chrono::Utc::now();
        Ok(node.clone())
    }

    async fn share_node(
        &self,
        tenant_id: &str,
        owner_id: &str,
        node_id: Ulid,
        with_user_id: &str,
    ) -> anyhow::Result<WorkspaceNode> {
        let mut nodes = self.nodes.lock().unwrap();
        let node = nodes
            .get_mut(&node_id)
            .filter(|n| n.tenant_id == tenant_id && n.owner_id == owner_id)
            .ok_or_else(|| {
                anyhow::anyhow!(crate::error::ConusAiError::NotFound(format!(
                    "node {node_id}"
                )))
            })?;
        if !node.shared_with.contains(&with_user_id.to_string()) {
            node.shared_with.push(with_user_id.to_string());
        }
        node.last_modified = Utc::now();
        Ok(node.clone())
    }

    async fn unshare_node(
        &self,
        tenant_id: &str,
        owner_id: &str,
        node_id: Ulid,
        with_user_id: &str,
    ) -> anyhow::Result<WorkspaceNode> {
        let mut nodes = self.nodes.lock().unwrap();
        let node = nodes
            .get_mut(&node_id)
            .filter(|n| n.tenant_id == tenant_id && n.owner_id == owner_id)
            .ok_or_else(|| {
                anyhow::anyhow!(crate::error::ConusAiError::NotFound(format!(
                    "node {node_id}"
                )))
            })?;
        node.shared_with.retain(|u| u != with_user_id);
        node.last_modified = Utc::now();
        Ok(node.clone())
    }

    async fn bump_last_modified(&self, _tenant_id: &str, node_id: Ulid) -> anyhow::Result<()> {
        if let Some(node) = self.nodes.lock().unwrap().get_mut(&node_id) {
            node.last_modified = Utc::now();
        }
        Ok(())
    }

    async fn search_nodes(
        &self,
        tenant_id: &str,
        user_id: &str,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<WorkspaceNode>> {
        let q = query.to_lowercase();
        let nodes = self.nodes.lock().unwrap();
        let content = self.content.lock().unwrap();
        let result: Vec<WorkspaceNode> = nodes
            .values()
            .filter(|n| n.tenant_id == tenant_id && Self::check_access(n, user_id))
            .filter(|n| {
                let name = n.name.to_lowercase();
                let body = content
                    .get(&n.id)
                    .map(|s| s.to_lowercase())
                    .unwrap_or_default();
                name.contains(&q) || body.contains(&q)
            })
            .take(limit)
            .cloned()
            .collect();
        Ok(result)
    }

    async fn index_content(
        &self,
        _tenant_id: &str,
        node_id: Ulid,
        content: &str,
    ) -> anyhow::Result<()> {
        self.content
            .lock()
            .unwrap()
            .insert(node_id, content.to_owned());
        Ok(())
    }

    async fn semantic_search_nodes(
        &self,
        tenant_id: &str,
        user_id: &str,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<WorkspaceNode>> {
        // In-memory store has no embedding engine — fall back to full-text search.
        self.search_nodes(tenant_id, user_id, query, limit).await
    }

    async fn bind_thread(
        &self,
        tenant_id: &str,
        node_id: Ulid,
        thread_id: &str,
    ) -> anyhow::Result<WorkspaceNode> {
        let mut nodes = self.nodes.lock().unwrap();
        let node = nodes
            .get_mut(&node_id)
            .filter(|n| n.tenant_id == tenant_id)
            .ok_or_else(|| {
                anyhow::anyhow!(crate::error::ConusAiError::NotFound(format!(
                    "node {node_id}"
                )))
            })?;
        let mut meta = match node.metadata.take() {
            serde_json::Value::Object(m) => m,
            _ => serde_json::Map::new(),
        };
        meta.insert(
            "thread_id".into(),
            serde_json::Value::String(thread_id.to_string()),
        );
        node.metadata = serde_json::Value::Object(meta);
        node.last_modified = Utc::now();
        Ok(node.clone())
    }

    async fn is_tenant_seeded(&self, tenant_id: &str) -> anyhow::Result<bool> {
        Ok(self.seeded.lock().unwrap().contains(tenant_id))
    }

    async fn mark_tenant_seeded(&self, tenant_id: &str) -> anyhow::Result<()> {
        self.seeded.lock().unwrap().insert(tenant_id.to_owned());
        Ok(())
    }

    async fn create_protected_root_folder(
        &self,
        tenant_id: &str,
        owner_id: &str,
        name: &str,
    ) -> anyhow::Result<WorkspaceNode> {
        validate_name(name, NodeKind::Folder).map_err(|e| anyhow::anyhow!("{e}"))?;
        let virtual_path = name.to_owned();
        let mut node = WorkspaceNode::new_folder(tenant_id, owner_id, None, name, virtual_path);
        node.is_protected_root = true;
        self.nodes.lock().unwrap().insert(node.id, node.clone());
        Ok(node)
    }

    async fn purge_tenant_data(&self, tenant_id: &str) -> anyhow::Result<()> {
        self.nodes
            .lock()
            .unwrap()
            .retain(|_, node| node.tenant_id != tenant_id);
        self.seeded.lock().unwrap().remove(tenant_id);
        Ok(())
    }
}

// ─── WorkspaceContentStore ────────────────────────────────────────────────────

#[derive(Default)]
pub struct InMemoryWorkspaceContent {
    store: Mutex<HashMap<(String, String), String>>,
}

impl InMemoryWorkspaceContent {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl WorkspaceContentStore for InMemoryWorkspaceContent {
    async fn read(&self, tenant_id: &str, virtual_path: &str) -> anyhow::Result<String> {
        Ok(self
            .store
            .lock()
            .unwrap()
            .get(&(tenant_id.to_owned(), virtual_path.to_owned()))
            .cloned()
            .unwrap_or_default())
    }

    async fn write(&self, tenant_id: &str, virtual_path: &str, body: &str) -> anyhow::Result<()> {
        self.store.lock().unwrap().insert(
            (tenant_id.to_owned(), virtual_path.to_owned()),
            body.to_owned(),
        );
        Ok(())
    }

    async fn delete(&self, tenant_id: &str, virtual_path: &str) -> anyhow::Result<()> {
        self.store
            .lock()
            .unwrap()
            .remove(&(tenant_id.to_owned(), virtual_path.to_owned()));
        Ok(())
    }
}

// ─── AuditStore ───────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct InMemoryAuditStore {
    events: Mutex<Vec<AuditEvent>>,
}

impl InMemoryAuditStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl AuditStore for InMemoryAuditStore {
    async fn append(&self, event: AuditEvent) -> crate::error::Result<()> {
        self.events.lock().unwrap().push(event);
        Ok(())
    }

    async fn list(
        &self,
        tenant_id: &str,
        limit: usize,
        after: Option<&str>,
    ) -> crate::error::Result<Vec<AuditEvent>> {
        let events = self.events.lock().unwrap();
        let mut result: Vec<AuditEvent> = events
            .iter()
            .filter(|e| e.tenant_id == tenant_id)
            .cloned()
            .collect();
        // Newest first
        result.sort_by_key(|e| Reverse(e.timestamp));
        // Apply cursor: skip events until we pass the one matching `after`
        if let Some(cursor) = after
            && let Some(pos) = result.iter().position(|e| e.id == cursor)
        {
            result = result.into_iter().skip(pos + 1).collect();
        }
        result.truncate(limit);
        Ok(result)
    }
}
