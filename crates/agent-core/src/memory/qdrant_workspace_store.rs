/// QdrantWorkspaceStore — index store for WorkspaceNode (folders, conversations, files).
///
/// Mirrors QdrantThreadStore exactly: HTTP REST, 4-dim zero vectors, keyword payload indexes.
/// One collection per tenant: `workspaces_{tenant_id}`.
///
/// Access control: every read filters on `tenant_id` AND (owner_id == U OR shared_with ∋ U).
/// Non-owners receive NotFound — existence is never leaked.
use async_trait::async_trait;
use chrono::Utc;
use common::error::ConusAiError;
use common::memory::store::WorkspaceStore;
use common::memory::workspace::{NodeKind, WorkspaceNode, join_virtual_path, validate_name};
use reqwest::Client;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use common::metrics;
use std::time::Instant;
use tracing::{Span, info, instrument, warn};
use ulid::Ulid;

const VECTOR_DIM: usize = 4;

fn zero_vec() -> Vec<f32> {
    vec![0.0; VECTOR_DIM]
}

fn point_id(key: &str) -> u64 {
    let mut h = Sha256::new();
    h.update(key.as_bytes());
    let digest = h.finalize();
    u64::from_le_bytes(digest[..8].try_into().unwrap())
}

pub struct QdrantWorkspaceStore {
    http: Client,
    base_url: String,
}

impl QdrantWorkspaceStore {
    pub fn new(qdrant_url: impl Into<String>) -> Self {
        Self {
            http: Client::new(),
            base_url: qdrant_url.into(),
        }
    }

    fn collection(&self, tenant_id: &str) -> String {
        format!("workspaces_{tenant_id}")
    }

    async fn ensure_collection(&self, tenant_id: &str) -> anyhow::Result<()> {
        let col = self.collection(tenant_id);
        let url = format!("{}/collections/{}", self.base_url, col);

        if self.http.get(&url).send().await?.status().is_success() {
            return Ok(());
        }

        let res = self
            .http
            .put(&url)
            .json(&json!({
                "vectors": { "size": VECTOR_DIM, "distance": "Cosine" }
            }))
            .send()
            .await?;

        if !res.status().is_success() {
            let body = res.text().await.unwrap_or_default();
            anyhow::bail!("failed to create workspace collection {col}: {body}");
        }

        for field in ["tenant_id", "owner_id", "parent_id", "kind", "shared_with"] {
            let idx_url = format!("{}/collections/{}/index", self.base_url, col);
            let _ = self
                .http
                .put(&idx_url)
                .json(&json!({
                    "field_name": field,
                    "field_schema": "keyword"
                }))
                .send()
                .await;
        }

        // Full-text indexes on name + content_text for search_nodes()
        for text_field in ["name", "content_text"] {
            let idx_url = format!("{}/collections/{}/index", self.base_url, col);
            let _ = self
                .http
                .put(&idx_url)
                .json(&json!({
                    "field_name": text_field,
                    "field_schema": {
                        "type": "text",
                        "tokenizer": "word",
                        "min_token_len": 2,
                        "max_token_len": 128,
                        "lowercase": true
                    }
                }))
                .send()
                .await;
        }

        info!(collection = col, "created Qdrant workspace collection");
        Ok(())
    }

    #[instrument(skip(self, point), fields(db.system = "qdrant", db.operation = "upsert", db.collection = tracing::field::Empty, error.type = tracing::field::Empty))]
    async fn upsert_point(&self, tenant_id: &str, point: Value) -> anyhow::Result<()> {
        let col = self.collection(tenant_id);
        Span::current().record("db.collection", col.as_str());
        let labels = [metrics::kv("operation", "upsert"), metrics::kv("collection", col.as_str())];
        let t0 = Instant::now();
        let url = format!("{}/collections/{}/points", self.base_url, col);
        let res = self.http.put(&url).json(&json!({ "points": [point] })).send().await?;
        metrics::qdrant_duration_ms().record(t0.elapsed().as_secs_f64() * 1000.0, &labels);
        if !res.status().is_success() {
            let body = res.text().await.unwrap_or_default();
            metrics::qdrant_errors().add(1, &labels);
            Span::current().record("error.type", body.as_str());
            anyhow::bail!("workspace upsert failed: {body}");
        }
        Ok(())
    }

    #[instrument(skip(self, filter), fields(db.system = "qdrant", db.operation = "scroll", db.collection = tracing::field::Empty, error.type = tracing::field::Empty))]
    async fn scroll_filter(
        &self,
        tenant_id: &str,
        filter: Value,
        limit: usize,
    ) -> anyhow::Result<Vec<Value>> {
        let col = self.collection(tenant_id);
        Span::current().record("db.collection", col.as_str());
        let labels = [metrics::kv("operation", "scroll"), metrics::kv("collection", col.as_str())];
        let t0 = Instant::now();
        let url = format!("{}/collections/{}/points/scroll", self.base_url, col);
        let res = self
            .http
            .post(&url)
            .json(&json!({ "filter": filter, "limit": limit, "with_payload": true, "with_vector": false }))
            .send()
            .await?;
        metrics::qdrant_duration_ms().record(t0.elapsed().as_secs_f64() * 1000.0, &labels);
        if !res.status().is_success() {
            let body = res.text().await.unwrap_or_default();
            metrics::qdrant_errors().add(1, &labels);
            Span::current().record("error.type", body.as_str());
            anyhow::bail!("workspace scroll failed: {body}");
        }
        let body: Value = res.json().await?;
        Ok(body["result"]["points"].as_array().cloned().unwrap_or_default())
    }

    #[instrument(skip(self), fields(db.system = "qdrant", db.operation = "delete", db.collection = tracing::field::Empty, error.type = tracing::field::Empty))]
    async fn delete_point(&self, tenant_id: &str, node_id: Ulid) -> anyhow::Result<()> {
        let col = self.collection(tenant_id);
        Span::current().record("db.collection", col.as_str());
        let labels = [metrics::kv("operation", "delete"), metrics::kv("collection", col.as_str())];
        let t0 = Instant::now();
        let url = format!("{}/collections/{}/points/delete", self.base_url, col);
        let pid = point_id(&node_id.to_string());
        let res = self.http.post(&url).json(&json!({ "points": [pid] })).send().await?;
        metrics::qdrant_duration_ms().record(t0.elapsed().as_secs_f64() * 1000.0, &labels);
        if !res.status().is_success() {
            let body = res.text().await.unwrap_or_default();
            metrics::qdrant_errors().add(1, &labels);
            Span::current().record("error.type", body.as_str());
            anyhow::bail!("workspace delete failed: {body}");
        }
        Ok(())
    }

    fn node_from_payload(p: &Value) -> Option<WorkspaceNode> {
        let payload = &p["payload"];
        let kind = match payload["kind"].as_str()? {
            "folder" => NodeKind::Folder,
            "conversation" => NodeKind::Conversation,
            "file" => NodeKind::File,
            _ => return None,
        };
        let id: Ulid = payload["id"].as_str()?.parse().ok()?;
        let parent_id = payload["parent_id"].as_str().and_then(|s| {
            if s == "null" || s.is_empty() {
                None
            } else {
                s.parse().ok()
            }
        });

        Some(WorkspaceNode {
            id,
            tenant_id: payload["tenant_id"].as_str()?.to_owned(),
            owner_id: payload["owner_id"].as_str()?.to_owned(),
            parent_id,
            kind,
            name: payload["name"].as_str()?.to_owned(),
            virtual_path: payload["virtual_path"].as_str()?.to_owned(),
            last_modified: payload["last_modified"]
                .as_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(Utc::now),
            shared_with: payload["shared_with"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            metadata: payload["metadata"].clone(),
        })
    }

    fn node_to_point(node: &WorkspaceNode) -> Value {
        let kind_str = match node.kind {
            NodeKind::Folder => "folder",
            NodeKind::Conversation => "conversation",
            NodeKind::File => "file",
        };
        json!({
            "id": point_id(&node.id.to_string()),
            "vector": zero_vec(),
            "payload": {
                "id": node.id.to_string(),
                "tenant_id": node.tenant_id,
                "owner_id": node.owner_id,
                "parent_id": node.parent_id.map(|u| u.to_string()).unwrap_or_default(),
                "kind": kind_str,
                "name": node.name,
                "virtual_path": node.virtual_path,
                "last_modified": node.last_modified.to_rfc3339(),
                "shared_with": node.shared_with,
                "metadata": node.metadata,
                // Seed content_text so new nodes are immediately searchable by name.
                // index_content() will overwrite this with real body content later.
                // Only used on initial creation — updates use patch_payload() instead.
                "content_text": "",
            }
        })
    }

    /// Targeted payload merge: sets only the given fields, never touches other keys
    /// (including content_text). Use this for all post-creation updates so indexed
    /// content is never clobbered by metadata operations.
    async fn patch_payload(
        &self,
        tenant_id: &str,
        node_id: Ulid,
        fields: Value,
    ) -> anyhow::Result<()> {
        let col = self.collection(tenant_id);
        let pid = point_id(&node_id.to_string());
        let url = format!("{}/collections/{}/points/payload", self.base_url, col);
        let labels = [
            metrics::kv("operation", "patch_payload"),
            metrics::kv("collection", col.as_str()),
        ];
        let t0 = Instant::now();
        let res = self
            .http
            .post(&url)
            .json(&json!({ "payload": fields, "points": [pid] }))
            .send()
            .await?;
        metrics::qdrant_duration_ms().record(t0.elapsed().as_secs_f64() * 1000.0, &labels);
        if !res.status().is_success() {
            let body = res.text().await.unwrap_or_default();
            metrics::qdrant_errors().add(1, &labels);
            anyhow::bail!("workspace patch_payload failed: {body}");
        }
        Ok(())
    }

    fn access_filter(tenant_id: &str, user_id: &str, extra: Vec<Value>) -> Value {
        let mut must = vec![json!({"key": "tenant_id", "match": {"value": tenant_id}})];
        must.extend(extra);
        json!({
            "must": must,
            "min_should": {
                "conditions": [
                    {"key": "owner_id", "match": {"value": user_id}},
                    {"key": "shared_with", "match": {"value": user_id}}
                ],
                "min_count": 1
            }
        })
    }

    /// Lazily ensure text indexes exist on collections that pre-date this feature.
    async fn ensure_text_indexes(&self, tenant_id: &str) {
        let col = self.collection(tenant_id);
        for text_field in ["name", "content_text"] {
            let idx_url = format!("{}/collections/{}/index", self.base_url, col);
            let _ = self
                .http
                .put(&idx_url)
                .json(&json!({
                    "field_name": text_field,
                    "field_schema": {
                        "type": "text",
                        "tokenizer": "word",
                        "min_token_len": 2,
                        "max_token_len": 128,
                        "lowercase": true
                    }
                }))
                .send()
                .await;
        }
    }

    /// Fetch a node directly by point_id (no access check — internal use only).
    async fn get_raw(
        &self,
        tenant_id: &str,
        node_id: Ulid,
    ) -> anyhow::Result<Option<WorkspaceNode>> {
        self.ensure_collection(tenant_id).await?;
        let col = self.collection(tenant_id);
        let pid = point_id(&node_id.to_string());
        let url = format!("{}/collections/{}/points/{}", self.base_url, col, pid);
        let res = self.http.get(&url).send().await?;
        if res.status().as_u16() == 404 {
            return Ok(None);
        }
        if !res.status().is_success() {
            anyhow::bail!(
                "workspace get failed: {}",
                res.text().await.unwrap_or_default()
            );
        }
        let body: Value = res.json().await?;
        Ok(Self::node_from_payload(&body["result"]))
    }
}

#[async_trait]
impl WorkspaceStore for QdrantWorkspaceStore {
    #[instrument(skip(self), fields(tenant_id, owner_id, name))]
    async fn create_folder(
        &self,
        tenant_id: &str,
        owner_id: &str,
        parent_id: Option<Ulid>,
        name: &str,
    ) -> anyhow::Result<WorkspaceNode> {
        validate_name(name, NodeKind::Folder).map_err(|e| anyhow::anyhow!("{e}"))?;
        self.ensure_collection(tenant_id).await?;

        let parent_path = if let Some(pid) = parent_id {
            let parent = self
                .get_raw(tenant_id, pid)
                .await?
                .ok_or_else(|| anyhow::anyhow!("parent not found"))?;
            Some(parent.virtual_path)
        } else {
            None
        };

        let virtual_path = join_virtual_path(parent_path.as_deref(), name);
        let node = WorkspaceNode::new_folder(tenant_id, owner_id, parent_id, name, virtual_path);
        let point = Self::node_to_point(&node);
        self.upsert_point(tenant_id, point).await?;
        info!(tenant_id, owner_id, name, "created workspace folder");
        Ok(node)
    }

    #[instrument(skip(self), fields(tenant_id, owner_id, name))]
    async fn create_conversation(
        &self,
        tenant_id: &str,
        owner_id: &str,
        parent_id: Option<Ulid>,
        name: &str,
    ) -> anyhow::Result<WorkspaceNode> {
        validate_name(name, NodeKind::Conversation).map_err(|e| anyhow::anyhow!("{e}"))?;
        self.ensure_collection(tenant_id).await?;

        let parent_path = if let Some(pid) = parent_id {
            let parent = self
                .get_raw(tenant_id, pid)
                .await?
                .ok_or_else(|| anyhow::anyhow!("parent not found"))?;
            Some(parent.virtual_path)
        } else {
            None
        };

        let virtual_path = join_virtual_path(parent_path.as_deref(), name);
        let node =
            WorkspaceNode::new_conversation(tenant_id, owner_id, parent_id, name, &virtual_path);
        let point = Self::node_to_point(&node);
        self.upsert_point(tenant_id, point).await?;
        info!(tenant_id, owner_id, name, "created workspace conversation");
        Ok(node)
    }

    #[instrument(skip(self), fields(tenant_id, user_id))]
    async fn list_accessible_children(
        &self,
        tenant_id: &str,
        user_id: &str,
        parent_id: Option<Ulid>,
    ) -> anyhow::Result<Vec<WorkspaceNode>> {
        self.ensure_collection(tenant_id).await?;

        let parent_val = match parent_id {
            Some(pid) => pid.to_string(),
            None => String::new(),
        };
        let extra = vec![json!({"key": "parent_id", "match": {"value": parent_val}})];
        let filter = Self::access_filter(tenant_id, user_id, extra);

        let points = self.scroll_filter(tenant_id, filter, 500).await?;
        let mut nodes: Vec<WorkspaceNode> =
            points.iter().filter_map(Self::node_from_payload).collect();
        nodes.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(nodes)
    }

    #[instrument(skip(self), fields(tenant_id, user_id, %id))]
    async fn get_accessible_node(
        &self,
        tenant_id: &str,
        user_id: &str,
        id: Ulid,
    ) -> anyhow::Result<WorkspaceNode> {
        let node = self
            .get_raw(tenant_id, id)
            .await?
            .ok_or_else(|| anyhow::anyhow!(ConusAiError::NotFound(format!("node {id}"))))?;

        let is_owner = node.owner_id == user_id;
        let is_shared = node.shared_with.iter().any(|u| u == user_id);
        if !is_owner && !is_shared {
            // Do not leak existence — return NotFound
            anyhow::bail!(ConusAiError::NotFound(format!("node {id}")));
        }
        Ok(node)
    }

    #[instrument(skip(self), fields(tenant_id, user_id, %node_id))]
    async fn get_ancestors(
        &self,
        tenant_id: &str,
        user_id: &str,
        node_id: Ulid,
    ) -> anyhow::Result<Vec<WorkspaceNode>> {
        let mut ancestors: Vec<WorkspaceNode> = vec![];
        let mut current_id = node_id;

        loop {
            let node = match self.get_raw(tenant_id, current_id).await? {
                Some(n) => n,
                None => break,
            };
            // Access-check the ancestor too
            let is_owner = node.owner_id == user_id;
            let is_shared = node.shared_with.iter().any(|u| u == user_id);
            if !is_owner && !is_shared {
                break; // stop climbing at first inaccessible ancestor
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

    #[instrument(skip(self), fields(tenant_id, user_id, %node_id))]
    async fn move_node(
        &self,
        tenant_id: &str,
        user_id: &str,
        node_id: Ulid,
        new_parent: Option<Ulid>,
        new_parent_path: Option<&str>,
    ) -> anyhow::Result<WorkspaceNode> {
        let mut node = self
            .get_accessible_node(tenant_id, user_id, node_id)
            .await?;

        let new_virtual_path = join_virtual_path(new_parent_path, &node.name);
        node.parent_id = new_parent;
        node.virtual_path = new_virtual_path.clone();
        node.last_modified = Utc::now();

        // Use patch_payload so content_text is not clobbered.
        self.patch_payload(tenant_id, node_id, json!({
            "parent_id": node.parent_id.map(|u| u.to_string()).unwrap_or_default(),
            "virtual_path": new_virtual_path,
            "last_modified": node.last_modified.to_rfc3339(),
        })).await?;
        Ok(node)
    }

    #[instrument(skip(self), fields(tenant_id, user_id, %node_id))]
    async fn delete_node(
        &self,
        tenant_id: &str,
        user_id: &str,
        node_id: Ulid,
    ) -> anyhow::Result<()> {
        // Verify access before doing anything destructive
        self.get_accessible_node(tenant_id, user_id, node_id)
            .await?;

        // Worklist-based recursive delete (avoids deep async recursion)
        let mut worklist: Vec<Ulid> = vec![node_id];
        while let Some(current_id) = worklist.pop() {
            // Collect children (no access filter — delete all children of owned root)
            let col = self.collection(tenant_id);
            let url = format!("{}/collections/{}/points/scroll", self.base_url, col);
            let res = self
                .http
                .post(&url)
                .json(&json!({
                    "filter": {
                        "must": [
                            {"key": "tenant_id", "match": {"value": tenant_id}},
                            {"key": "parent_id", "match": {"value": current_id.to_string()}}
                        ]
                    },
                    "limit": 500,
                    "with_payload": true,
                    "with_vector": false
                }))
                .send()
                .await?;

            if res.status().is_success() {
                let body: Value = res.json().await?;
                for child in body["result"]["points"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                {
                    if let Some(child_node) = Self::node_from_payload(&child) {
                        worklist.push(child_node.id);
                    }
                }
            }

            self.delete_point(tenant_id, current_id).await?;
        }
        Ok(())
    }

    #[instrument(skip(self), fields(tenant_id, owner_id, %node_id, with_user_id))]
    async fn share_node(
        &self,
        tenant_id: &str,
        owner_id: &str,
        node_id: Ulid,
        with_user_id: &str,
    ) -> anyhow::Result<WorkspaceNode> {
        let mut node = self
            .get_raw(tenant_id, node_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!(ConusAiError::NotFound(format!("node {node_id}"))))?;

        if node.owner_id != owner_id {
            anyhow::bail!(ConusAiError::NotFound(format!("node {node_id}")));
        }
        if !node.shared_with.contains(&with_user_id.to_string()) {
            node.shared_with.push(with_user_id.to_string());
        }
        node.last_modified = Utc::now();
        // Use patch_payload so content_text is not clobbered.
        self.patch_payload(tenant_id, node_id, json!({
            "shared_with": node.shared_with,
            "last_modified": node.last_modified.to_rfc3339(),
        })).await?;
        Ok(node)
    }

    #[instrument(skip(self), fields(tenant_id, owner_id, %node_id, with_user_id))]
    async fn unshare_node(
        &self,
        tenant_id: &str,
        owner_id: &str,
        node_id: Ulid,
        with_user_id: &str,
    ) -> anyhow::Result<WorkspaceNode> {
        let mut node = self
            .get_raw(tenant_id, node_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!(ConusAiError::NotFound(format!("node {node_id}"))))?;

        if node.owner_id != owner_id {
            anyhow::bail!(ConusAiError::NotFound(format!("node {node_id}")));
        }
        node.shared_with.retain(|u| u != with_user_id);
        node.last_modified = Utc::now();
        // Use patch_payload so content_text is not clobbered.
        self.patch_payload(tenant_id, node_id, json!({
            "shared_with": node.shared_with,
            "last_modified": node.last_modified.to_rfc3339(),
        })).await?;
        Ok(node)
    }

    #[instrument(skip(self), fields(tenant_id, %node_id, thread_id))]
    async fn bind_thread(
        &self,
        tenant_id: &str,
        node_id: Ulid,
        thread_id: &str,
    ) -> anyhow::Result<WorkspaceNode> {
        let mut node = self
            .get_raw(tenant_id, node_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!(ConusAiError::NotFound(format!("node {node_id}"))))?;

        // Merge into existing metadata object (preserve other keys).
        let mut meta = match node.metadata.take() {
            Value::Object(map) => map,
            _ => serde_json::Map::new(),
        };
        meta.insert("thread_id".into(), Value::String(thread_id.to_string()));
        node.metadata = Value::Object(meta);
        node.last_modified = Utc::now();

        // Use patch_payload so content_text is not clobbered.
        self.patch_payload(tenant_id, node_id, json!({
            "metadata": node.metadata,
            "last_modified": node.last_modified.to_rfc3339(),
        })).await?;
        Ok(node)
    }

    #[instrument(skip(self), fields(tenant_id, %node_id))]
    async fn bump_last_modified(&self, tenant_id: &str, node_id: Ulid) -> anyhow::Result<()> {
        if self.get_raw(tenant_id, node_id).await?.is_some() {
            self.patch_payload(tenant_id, node_id, json!({
                "last_modified": Utc::now().to_rfc3339(),
            })).await?;
        } else {
            warn!(%node_id, "bump_last_modified: node not found, skipping");
        }
        Ok(())
    }

    #[instrument(skip(self), fields(tenant_id, query, limit))]
    async fn search_nodes(
        &self,
        tenant_id: &str,
        user_id: &str,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<WorkspaceNode>> {
        self.ensure_collection(tenant_id).await?;
        // Ensure text indexes exist on collections that pre-date this feature.
        self.ensure_text_indexes(tenant_id).await;

        let col = self.collection(tenant_id);
        let url = format!("{}/collections/{}/points/scroll", self.base_url, col);

        // Build a filter: access control AND (name OR content_text) text_match on each token.
        // Qdrant text_match is per-token; we use min_should so ANY token match in name OR content
        // bubbles up the node, and we require all tokens to appear somewhere (name or content).
        let query_lower = query.to_lowercase();
        let tokens: Vec<&str> = query_lower.split_whitespace().collect();

        // For each query token, the node must match it in name OR content_text.
        let token_conditions: Vec<Value> = tokens
            .iter()
            .map(|token| {
                json!({
                    "should": [
                        {"key": "name",         "match": {"text": *token}},
                        {"key": "content_text", "match": {"text": *token}}
                    ]
                })
            })
            .collect();

        let mut must = vec![
            json!({"key": "tenant_id", "match": {"value": tenant_id}}),
        ];
        must.extend(token_conditions);

        let filter = json!({
            "must": must,
            "min_should": {
                "conditions": [
                    {"key": "owner_id", "match": {"value": user_id}},
                    {"key": "shared_with", "match": {"value": user_id}}
                ],
                "min_count": 1
            }
        });

        let labels = [
            metrics::kv("operation", "scroll"),
            metrics::kv("collection", col.as_str()),
        ];
        let t0 = Instant::now();
        let res = self
            .http
            .post(&url)
            .json(&json!({
                "filter": filter,
                "limit": limit,
                "with_payload": true,
                "with_vector": false
            }))
            .send()
            .await?;
        metrics::qdrant_duration_ms().record(t0.elapsed().as_secs_f64() * 1000.0, &labels);

        if !res.status().is_success() {
            let body = res.text().await.unwrap_or_default();
            metrics::qdrant_errors().add(1, &labels);
            // If the text index isn't ready yet, fall back to fetching all and filtering locally.
            warn!(error = %body, "Qdrant text search failed; falling back to substring scan");
            return self.search_nodes_fallback(tenant_id, user_id, query, limit).await;
        }

        let body: Value = res.json().await?;
        let nodes: Vec<WorkspaceNode> = body["result"]["points"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(Self::node_from_payload)
            .collect();

        // If Qdrant returns nothing (e.g. index not yet built), fall back.
        if nodes.is_empty() && !query.is_empty() {
            return self.search_nodes_fallback(tenant_id, user_id, query, limit).await;
        }

        Ok(nodes)
    }

    #[instrument(skip(self, content), fields(tenant_id, node_id))]
    async fn index_content(
        &self,
        tenant_id: &str,
        node_id: Ulid,
        content: &str,
    ) -> anyhow::Result<()> {
        // Truncate to ~32 KB to avoid oversized Qdrant payloads.
        const MAX_BYTES: usize = 32 * 1024;
        let snippet: &str = if content.len() > MAX_BYTES {
            // Trim to a clean UTF-8 boundary.
            let boundary = content
                .char_indices()
                .take_while(|(i, _)| *i < MAX_BYTES)
                .last()
                .map(|(i, c)| i + c.len_utf8())
                .unwrap_or(MAX_BYTES);
            &content[..boundary]
        } else {
            content
        };

        // Verify node exists (no-op if already deleted).
        if self.get_raw(tenant_id, node_id).await?.is_none() {
            return Ok(());
        }

        // Use a targeted payload SET — this merges fields rather than replacing the whole
        // point, so we preserve all existing payload keys (name, kind, owner_id, etc.).
        // We set both content_text and last_modified in one call so there's no race
        // between a partial set and a full upsert.
        let col = self.collection(tenant_id);
        let pid = point_id(&node_id.to_string());
        let url = format!("{}/collections/{}/points/payload", self.base_url, col);

        let labels = [
            metrics::kv("operation", "payload"),
            metrics::kv("collection", col.as_str()),
        ];
        let t0 = Instant::now();
        let res = self
            .http
            .post(&url)
            .json(&json!({
                "payload": {
                    "content_text": snippet,
                    "last_modified": Utc::now().to_rfc3339()
                },
                "points": [pid]
            }))
            .send()
            .await?;
        metrics::qdrant_duration_ms().record(t0.elapsed().as_secs_f64() * 1000.0, &labels);

        if !res.status().is_success() {
            let body = res.text().await.unwrap_or_default();
            metrics::qdrant_errors().add(1, &labels);
            anyhow::bail!("content index update failed: {body}");
        }

        Ok(())
    }
}

impl QdrantWorkspaceStore {
    /// Substring fallback: scroll all nodes the user can see and filter in Rust.
    /// Matches on both `name` and `content_text` payload fields.
    async fn search_nodes_fallback(
        &self,
        tenant_id: &str,
        user_id: &str,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<WorkspaceNode>> {
        let filter = Self::access_filter(tenant_id, user_id, vec![]);
        let all = self.scroll_filter(tenant_id, filter, 1000).await?;
        let q = query.to_lowercase();
        let matches: Vec<WorkspaceNode> = all
            .iter()
            .filter(|p| {
                let payload = &p["payload"];
                let name = payload["name"].as_str().unwrap_or("").to_lowercase();
                let content = payload["content_text"].as_str().unwrap_or("").to_lowercase();
                name.contains(&q) || content.contains(&q)
            })
            .filter_map(Self::node_from_payload)
            .take(limit)
            .collect();
        Ok(matches)
    }
}
