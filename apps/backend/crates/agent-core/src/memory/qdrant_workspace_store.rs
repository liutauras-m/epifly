/// QdrantWorkspaceStore — index store for WorkspaceNode (folders, conversations, files).
///
/// One collection per tenant: `workspaces_{tenant_id}`.
/// Vectors are 4-dim zeros; Qdrant is used as a document store + keyword/text search engine.
///
/// Access control: every read filters on `tenant_id` AND (owner_id == U OR shared_with ∋ U).
/// Non-owners receive NotFound — existence is never leaked.
use super::qdrant_helpers::{VECTOR_DIM, payload_to_json, point_id};
use async_trait::async_trait;
use chrono::Utc;
use common::error::ConusAiError;
use common::memory::store::WorkspaceStore;
use common::memory::workspace::{NodeKind, WorkspaceNode, join_virtual_path, validate_name};
use common::metrics;
use qdrant_client::Qdrant;
use qdrant_client::qdrant::{
    Condition, CreateCollectionBuilder, CreateFieldIndexCollectionBuilder, DeletePointsBuilder,
    Distance, FieldType, Filter, GetPointsBuilder, MinShould, PointStruct, ScrollPointsBuilder,
    SetPayloadPointsBuilder, UpsertPointsBuilder, VectorParamsBuilder,
};
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Instant;
use tracing::{Span, info, instrument, warn};
use ulid::Ulid;

pub struct QdrantWorkspaceStore {
    client: Arc<Qdrant>,
}

impl QdrantWorkspaceStore {
    pub fn new(grpc_url: impl Into<String>) -> Self {
        let client = Arc::new(
            Qdrant::from_url(&grpc_url.into())
                .build()
                .expect("qdrant-client build failed"),
        );
        Self { client }
    }

    fn collection(&self, tenant_id: &str) -> String {
        format!("workspaces_{tenant_id}")
    }

    async fn ensure_collection(&self, tenant_id: &str) -> anyhow::Result<()> {
        let col = self.collection(tenant_id);
        if self.client.collection_exists(&col).await? {
            return Ok(());
        }
        self.client
            .create_collection(
                CreateCollectionBuilder::new(&col)
                    .vectors_config(VectorParamsBuilder::new(VECTOR_DIM as u64, Distance::Cosine)),
            )
            .await?;
        for field in &["tenant_id", "owner_id", "parent_id", "kind", "shared_with"] {
            let _ = self
                .client
                .create_field_index(CreateFieldIndexCollectionBuilder::new(
                    &col,
                    *field,
                    FieldType::Keyword,
                ))
                .await;
        }
        tracing::info!(collection = col.as_str(), "created Qdrant collection");
        Ok(())
    }

    async fn upsert_point(&self, tenant_id: &str, point: Value) -> anyhow::Result<()> {
        let col = self.collection(tenant_id);
        let pid: u64 = point["id"]
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("missing point id"))?;
        let vector: Vec<f32> = point["vector"]
            .as_array()
            .map(|a| a.iter().map(|v| v.as_f64().unwrap_or(0.0) as f32).collect())
            .unwrap_or_else(|| vec![0.0_f32; VECTOR_DIM]);
        let payload: qdrant_client::Payload = point["payload"]
            .clone()
            .try_into()
            .map_err(|e| anyhow::anyhow!("payload conversion: {e:?}"))?;
        self.client
            .upsert_points(
                UpsertPointsBuilder::new(&col, vec![PointStruct::new(pid, vector, payload)])
                    .wait(true),
            )
            .await?;
        Ok(())
    }

    async fn scroll_filter(
        &self,
        tenant_id: &str,
        filter: Filter,
        limit: usize,
    ) -> anyhow::Result<Vec<Value>> {
        let col = self.collection(tenant_id);
        let resp = self
            .client
            .scroll(
                ScrollPointsBuilder::new(&col)
                    .filter(filter)
                    .limit(limit as u32)
                    .with_payload(true)
                    .with_vectors(false),
            )
            .await?;
        Ok(resp
            .result
            .into_iter()
            .map(|p| json!({ "payload": payload_to_json(p.payload) }))
            .collect())
    }

    async fn delete_point_by_ulid(&self, tenant_id: &str, node_id: Ulid) -> anyhow::Result<()> {
        let col = self.collection(tenant_id);
        let pid = point_id(&node_id.to_string());
        self.client
            .delete_points(
                DeletePointsBuilder::new(&col)
                    .points(Filter::must([Condition::has_id([pid])])),
            )
            .await?;
        Ok(())
    }

    async fn patch_payload(
        &self,
        tenant_id: &str,
        node_id: Ulid,
        fields: Value,
    ) -> anyhow::Result<()> {
        let col = self.collection(tenant_id);
        let pid = point_id(&node_id.to_string());
        let payload: qdrant_client::Payload = fields
            .try_into()
            .map_err(|e| anyhow::anyhow!("patch payload conversion: {e:?}"))?;
        self.client
            .set_payload(
                SetPayloadPointsBuilder::new(&col, payload)
                    .points_selector(Filter::must([Condition::has_id([pid])])),
            )
            .await?;
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
            "vector": vec![0.0_f32; VECTOR_DIM],
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
                "content_text": "",
            }
        })
    }

    fn access_filter(tenant_id: &str, user_id: &str, extra: Vec<Condition>) -> Filter {
        let mut must = vec![Condition::matches("tenant_id", tenant_id.to_string())];
        must.extend(extra);
        Filter {
            must,
            min_should: Some(MinShould {
                min_count: 1,
                conditions: vec![
                    Condition::matches("owner_id", user_id.to_string()),
                    Condition::matches("shared_with", user_id.to_string()),
                ],
            }),
            ..Default::default()
        }
    }

    /// Lazily ensure text indexes exist on collections that pre-date this feature.
    async fn ensure_text_indexes(&self, tenant_id: &str) {
        let col = self.collection(tenant_id);
        for field in &["name", "content_text"] {
            let _ = self
                .client
                .create_field_index(CreateFieldIndexCollectionBuilder::new(
                    &col,
                    *field,
                    FieldType::Text,
                ))
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
        let resp = self
            .client
            .get_points(
                GetPointsBuilder::new(&col, vec![pid.into()])
                    .with_payload(true)
                    .with_vectors(false),
            )
            .await?;
        let raw = resp.result.into_iter().next().map(|p| {
            json!({ "payload": payload_to_json(p.payload) })
        });
        Ok(raw.as_ref().and_then(Self::node_from_payload))
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
        let extra = vec![Condition::matches("parent_id", parent_val)];
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
            let is_owner = node.owner_id == user_id;
            let is_shared = node.shared_with.iter().any(|u| u == user_id);
            if !is_owner && !is_shared {
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

        self.patch_payload(
            tenant_id,
            node_id,
            json!({
                "parent_id": node.parent_id.map(|u| u.to_string()).unwrap_or_default(),
                "virtual_path": new_virtual_path,
                "last_modified": node.last_modified.to_rfc3339(),
            }),
        )
        .await?;
        Ok(node)
    }

    #[instrument(skip(self), fields(tenant_id, user_id, %node_id))]
    async fn delete_node(
        &self,
        tenant_id: &str,
        user_id: &str,
        node_id: Ulid,
    ) -> anyhow::Result<()> {
        self.get_accessible_node(tenant_id, user_id, node_id)
            .await?;

        // Worklist-based recursive delete (avoids deep async recursion)
        let mut worklist: Vec<Ulid> = vec![node_id];
        while let Some(current_id) = worklist.pop() {
            let children = self
                .scroll_filter(
                    tenant_id,
                    Filter::must([
                        Condition::matches("tenant_id", tenant_id.to_string()),
                        Condition::matches("parent_id", current_id.to_string()),
                    ]),
                    500,
                )
                .await
                .unwrap_or_default();

            for child in children {
                if let Some(child_node) = Self::node_from_payload(&child) {
                    worklist.push(child_node.id);
                }
            }

            self.delete_point_by_ulid(tenant_id, current_id).await?;
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
        self.patch_payload(
            tenant_id,
            node_id,
            json!({
                "shared_with": node.shared_with,
                "last_modified": node.last_modified.to_rfc3339(),
            }),
        )
        .await?;
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
        self.patch_payload(
            tenant_id,
            node_id,
            json!({
                "shared_with": node.shared_with,
                "last_modified": node.last_modified.to_rfc3339(),
            }),
        )
        .await?;
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

        let mut meta = match node.metadata.take() {
            Value::Object(map) => map,
            _ => serde_json::Map::new(),
        };
        meta.insert("thread_id".into(), Value::String(thread_id.to_string()));
        node.metadata = Value::Object(meta);
        node.last_modified = Utc::now();

        self.patch_payload(
            tenant_id,
            node_id,
            json!({
                "metadata": node.metadata,
                "last_modified": node.last_modified.to_rfc3339(),
            }),
        )
        .await?;
        Ok(node)
    }

    #[instrument(skip(self), fields(tenant_id, %node_id))]
    async fn bump_last_modified(&self, tenant_id: &str, node_id: Ulid) -> anyhow::Result<()> {
        if self.get_raw(tenant_id, node_id).await?.is_some() {
            self.patch_payload(
                tenant_id,
                node_id,
                json!({ "last_modified": Utc::now().to_rfc3339() }),
            )
            .await?;
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
        self.ensure_text_indexes(tenant_id).await;

        let col = self.collection(tenant_id);
        let query_lower = query.to_lowercase();
        let tokens: Vec<&str> = query_lower.split_whitespace().collect();

        // Build per-token conditions: each token must match name or content_text
        let token_conditions: Vec<Condition> = tokens
            .iter()
            .map(|token| {
                Condition::from(Filter::should([
                    Condition::matches_text("name", token.to_string()),
                    Condition::matches_text("content_text", token.to_string()),
                ]))
            })
            .collect();

        let mut must = vec![Condition::matches("tenant_id", tenant_id.to_string())];
        must.extend(token_conditions);

        let filter = Filter {
            must,
            min_should: Some(MinShould {
                min_count: 1,
                conditions: vec![
                    Condition::matches("owner_id", user_id.to_string()),
                    Condition::matches("shared_with", user_id.to_string()),
                ],
            }),
            ..Default::default()
        };

        let labels = [
            metrics::kv("operation", "scroll"),
            metrics::kv("collection", col.as_str()),
        ];
        let t0 = Instant::now();
        let res = self.scroll_filter(tenant_id, filter, limit).await;
        metrics::qdrant_duration_ms().record(t0.elapsed().as_secs_f64() * 1000.0, &labels);

        match res {
            Err(e) => {
                metrics::qdrant_errors().add(1, &labels);
                warn!(error = %e, "Qdrant text search failed; falling back to substring scan");
                self.search_nodes_fallback(tenant_id, user_id, query, limit)
                    .await
            }
            Ok(points) => {
                let nodes: Vec<WorkspaceNode> =
                    points.iter().filter_map(Self::node_from_payload).collect();
                if nodes.is_empty() && !query.is_empty() {
                    return self
                        .search_nodes_fallback(tenant_id, user_id, query, limit)
                        .await;
                }
                Ok(nodes)
            }
        }
    }

    #[instrument(skip(self, content), fields(tenant_id, node_id))]
    async fn index_content(
        &self,
        tenant_id: &str,
        node_id: Ulid,
        content: &str,
    ) -> anyhow::Result<()> {
        const MAX_BYTES: usize = 32 * 1024;
        let snippet: &str = if content.len() > MAX_BYTES {
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

        if self.get_raw(tenant_id, node_id).await?.is_none() {
            return Ok(());
        }

        let col = self.collection(tenant_id);
        let pid = point_id(&node_id.to_string());

        let labels = [
            metrics::kv("operation", "payload"),
            metrics::kv("collection", col.as_str()),
        ];
        let t0 = Instant::now();
        let payload: qdrant_client::Payload = json!({
            "content_text": snippet,
            "last_modified": Utc::now().to_rfc3339()
        })
        .try_into()
        .map_err(|e| anyhow::anyhow!("index payload conversion: {e:?}"))?;
        let res = self
            .client
            .set_payload(
                SetPayloadPointsBuilder::new(&col, payload)
                    .points_selector(Filter::must([Condition::has_id([pid])])),
            )
            .await;
        metrics::qdrant_duration_ms().record(t0.elapsed().as_secs_f64() * 1000.0, &labels);

        if let Err(e) = res {
            metrics::qdrant_errors().add(1, &labels);
            Span::current().record("error.type", e.to_string().as_str());
            anyhow::bail!("content index update failed: {e}");
        }
        Ok(())
    }
}

impl QdrantWorkspaceStore {
    /// Substring fallback: scroll all nodes the user can see and filter in Rust.
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
                let content = payload["content_text"]
                    .as_str()
                    .unwrap_or("")
                    .to_lowercase();
                name.contains(&q) || content.contains(&q)
            })
            .filter_map(Self::node_from_payload)
            .take(limit)
            .collect();
        Ok(matches)
    }
}
