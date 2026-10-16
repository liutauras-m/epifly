//! Native-storage capability providers over the tenant workspace (redb tree +
//! per-tenant object storage). There is no local-filesystem storage: every tool
//! operates on the calling tenant's `WorkspaceStore`/`WorkspaceContentStore`.
//!
//! `NativeStorageFactory` dispatches on the manifest name (`storage-workspace`) or
//! the `[config] op` field.

use crate::capabilities::card::CapabilityCard;
use crate::capabilities::manifest::{ToolKind, ToolManifest};
use crate::capabilities::provider::{CapabilityFactory, CapabilityProvider};
use crate::context::tenant::TenantContext;
use async_trait::async_trait;
use common::memory::store::{WorkspaceContentStore, WorkspaceStore};
use common::memory::workspace::NodeKind;
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::warn;

// ── StaticPlanProvider ────────────────────────────────────────────────────────

/// Returns a hard-coded `Vec<PlanStep>` from its manifest `[config] steps` array.
///
/// Used for `plan-on-upload` (and similar) capabilities declared entirely in TOML:
///
/// ```toml
/// [config]
/// op = "plan"
/// [[config.steps]]
/// capability = "extract.ocr.vision"
/// tool       = "ocr"
/// strategy   = "single"
/// [config.steps.input]
/// image_path = "{object_key}"
/// ```
pub struct StaticPlanProvider {
    manifest: ToolManifest,
    steps_json: Value,
}

impl StaticPlanProvider {
    pub fn new(manifest: ToolManifest) -> Self {
        let steps_json = manifest.config["steps"].clone();
        Self {
            manifest,
            steps_json,
        }
    }
}

#[async_trait]
impl CapabilityProvider for StaticPlanProvider {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn invoke(
        &self,
        tool_name: &str,
        _input: &Value,
        _tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        if tool_name != "plan" && tool_name != "steps" {
            anyhow::bail!("unknown tool: {tool_name}");
        }
        Ok(self.steps_json.clone())
    }
}

// ── StorageWorkspaceProvider ──────────────────────────────────────────────────

/// The user-facing workspace toolkit — 11 tools in one `CapabilityProvider`.
/// Dispatches on `tool_name` in `invoke()`; no `[config] op` needed.
pub struct StorageWorkspaceProvider {
    manifest: ToolManifest,
    workspace_store: Arc<dyn WorkspaceStore>,
    workspace_content: Arc<dyn WorkspaceContentStore>,
}

impl StorageWorkspaceProvider {
    pub fn new(
        manifest: ToolManifest,
        workspace_store: Arc<dyn WorkspaceStore>,
        workspace_content: Arc<dyn WorkspaceContentStore>,
    ) -> Self {
        Self {
            manifest,
            workspace_store,
            workspace_content,
        }
    }
}

#[async_trait]
impl CapabilityProvider for StorageWorkspaceProvider {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        let t = tenant.ok_or_else(|| anyhow::anyhow!("workspace tools require tenant context"))?;
        let tenant_id = t.tenant_id.as_str();
        let user_id = t
            .user_id
            .as_ref()
            .map(|u| u.as_str())
            .ok_or_else(|| anyhow::anyhow!("workspace tools require a user identity"))?;

        match tool_name {
            "save_document" => self.save_document(input, tenant_id, user_id).await,
            "list_folders" => self.list_folders(tenant_id, user_id).await,
            "show_tree" => self.show_tree(input, tenant_id, user_id).await,
            "find_by_name" => self.find_by_name(input, tenant_id, user_id).await,
            "create_folder" => self.create_folder(input, tenant_id, user_id).await,
            "move_node" => self.move_node(input, tenant_id, user_id).await,
            "delete_node" => self.delete_node(input, tenant_id, user_id).await,
            "bulk_delete" => self.bulk_delete(input, tenant_id, user_id).await,
            other => anyhow::bail!("unknown tool '{other}' for storage-workspace"),
        }
    }
}

impl StorageWorkspaceProvider {
    async fn save_document(
        &self,
        input: &Value,
        tenant_id: &str,
        user_id: &str,
    ) -> anyhow::Result<Value> {
        let folder_name = input["folder_name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("folder_name is required"))?
            .trim();
        let filename = input["filename"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("filename is required"))?
            .trim();
        let content = input["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("content is required"))?;
        if folder_name.is_empty() || filename.is_empty() {
            anyhow::bail!("folder_name and filename must not be empty");
        }
        // Preserve any explicit extension (e.g. "package.json", "svelte.config.js").
        // Only append ".md" when the caller passed a bare name with no dot.
        let doc_name = if filename.contains('.') {
            filename.to_string()
        } else {
            format!("{filename}.md")
        };
        let folders = self
            .workspace_store
            .list_accessible_children(tenant_id, user_id, None)
            .await
            .unwrap_or_default();
        let folder = if let Some(f) = folders
            .iter()
            .find(|n| n.kind == NodeKind::Folder && n.name.eq_ignore_ascii_case(folder_name))
        {
            f.clone()
        } else {
            self.workspace_store
                .create_folder(tenant_id, user_id, None, folder_name)
                .await
                .map_err(|e| anyhow::anyhow!("create folder '{folder_name}': {e}"))?
        };
        let node = self
            .workspace_store
            .create_conversation(tenant_id, user_id, Some(folder.id), &doc_name)
            .await
            .map_err(|e| anyhow::anyhow!("create document '{doc_name}': {e}"))?;
        if let Err(e) = self
            .workspace_content
            .write(tenant_id, &node.virtual_path, content)
            .await
        {
            warn!(error = %e, path = node.virtual_path, "save_document: content write failed");
        }
        Ok(json!({
            "status": "ok",
            "node_id": node.id.to_string(),
            "path": node.virtual_path,
            "folder": folder_name,
            "filename": doc_name,
            "bytes": content.len()
        }))
    }

    async fn list_folders(&self, tenant_id: &str, user_id: &str) -> anyhow::Result<Value> {
        let nodes = self
            .workspace_store
            .list_accessible_children(tenant_id, user_id, None)
            .await
            .unwrap_or_default();
        let folders: Vec<Value> = nodes
            .into_iter()
            .filter(|n| n.kind == NodeKind::Folder)
            .map(|n| json!({ "id": n.id.to_string(), "name": n.name, "path": n.virtual_path }))
            .collect();
        Ok(json!({ "folders": folders, "count": folders.len() }))
    }

    async fn show_tree(
        &self,
        input: &Value,
        tenant_id: &str,
        user_id: &str,
    ) -> anyhow::Result<Value> {
        let root_id: Option<ulid::Ulid> = input["parent_id"].as_str().and_then(|s| s.parse().ok());
        let max_depth = input["depth"].as_u64().unwrap_or(2).min(5) as usize;
        let mut buf = String::new();
        let mut count = 0_usize;
        Box::pin(self.render_subtree(
            tenant_id, user_id, root_id, 0, max_depth, &mut buf, &mut count,
        ))
        .await?;
        if buf.is_empty() {
            buf.push_str("_(workspace is empty)_");
        }
        Ok(json!({ "tree": buf, "node_count": count }))
    }

    // Recursive helper; the arguments are the traversal state, not an API.
    #[allow(clippy::too_many_arguments)]
    fn render_subtree<'a>(
        &'a self,
        tenant_id: &'a str,
        user_id: &'a str,
        parent: Option<ulid::Ulid>,
        depth: usize,
        max_depth: usize,
        out: &'a mut String,
        count: &'a mut usize,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send + 'a>> {
        Box::pin(async move {
            if depth > max_depth {
                return Ok(());
            }
            let children = self
                .workspace_store
                .list_accessible_children(tenant_id, user_id, parent)
                .await
                .unwrap_or_default();
            for child in children {
                *count += 1;
                let indent = "  ".repeat(depth);
                let icon = match child.kind {
                    NodeKind::Folder => "📁",
                    NodeKind::Conversation => "📄",
                    _ => "•",
                };
                out.push_str(&format!("{indent}- {icon} {}\n", child.name));
                if child.kind == NodeKind::Folder {
                    Box::pin(self.render_subtree(
                        tenant_id,
                        user_id,
                        Some(child.id),
                        depth + 1,
                        max_depth,
                        out,
                        count,
                    ))
                    .await?;
                }
            }
            Ok(())
        })
    }

    async fn find_by_name(
        &self,
        input: &Value,
        tenant_id: &str,
        user_id: &str,
    ) -> anyhow::Result<Value> {
        let name = input["name"]
            .as_str()
            .or_else(|| input["query"].as_str())
            .ok_or_else(|| anyhow::anyhow!("missing required field: name"))?;
        let limit = input["limit"]
            .as_u64()
            .filter(|n| *n > 0 && *n <= 50)
            .unwrap_or(10) as usize;
        let hits = self
            .workspace_store
            .search_nodes(tenant_id, user_id, name, limit)
            .await
            .map_err(|e| anyhow::anyhow!("search '{name}': {e}"))?;
        if hits.is_empty() {
            return Ok(json!({ "found": false, "matches": [] }));
        }
        let mut scored: Vec<(usize, &common::memory::workspace::WorkspaceNode)> = hits
            .iter()
            .map(|n| {
                let score = if n.name.eq_ignore_ascii_case(name) {
                    1000
                } else if n.name.to_lowercase().contains(&name.to_lowercase()) {
                    100
                } else {
                    1
                };
                (score, n)
            })
            .collect();
        scored.sort_by_key(|(s, _)| std::cmp::Reverse(*s));
        let best = scored[0].1;
        let matches: Vec<Value> = hits
            .iter()
            .map(|n| {
                json!({
                    "id": n.id.to_string(),
                    "name": n.name,
                    "kind": format!("{:?}", n.kind).to_lowercase(),
                    "virtual_path": n.virtual_path,
                    "parent_id": n.parent_id.map(|p| p.to_string()),
                })
            })
            .collect();
        Ok(json!({
            "found": true,
            "best_match": {
                "id": best.id.to_string(),
                "name": best.name,
                "kind": format!("{:?}", best.kind).to_lowercase(),
                "virtual_path": best.virtual_path,
                "parent_id": best.parent_id.map(|p| p.to_string()),
            },
            "matches": matches,
        }))
    }

    async fn create_folder(
        &self,
        input: &Value,
        tenant_id: &str,
        user_id: &str,
    ) -> anyhow::Result<Value> {
        let name = input["name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing required field: name"))?
            .trim();
        if name.is_empty() {
            anyhow::bail!("name must not be empty");
        }
        let parent_id: Option<ulid::Ulid> = match input.get("parent_id") {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) if s.is_empty() => None,
            Some(Value::String(s)) => Some(
                s.parse()
                    .map_err(|e| anyhow::anyhow!("invalid parent_id '{s}': {e}"))?,
            ),
            Some(other) => anyhow::bail!("parent_id must be a string ULID, got {other:?}"),
        };
        let folder = self
            .workspace_store
            .create_folder(tenant_id, user_id, parent_id, name)
            .await
            .map_err(|e| anyhow::anyhow!("create_folder '{name}': {e}"))?;
        Ok(json!({
            "status": "created",
            "id": folder.id.to_string(),
            "name": folder.name,
            "virtual_path": folder.virtual_path,
            "parent_id": folder.parent_id.map(|p| p.to_string()),
        }))
    }

    async fn move_node(
        &self,
        input: &Value,
        tenant_id: &str,
        user_id: &str,
    ) -> anyhow::Result<Value> {
        let id_str = if let Some(s) = input["id"].as_str().or_else(|| input["node_id"].as_str()) {
            s.to_string()
        } else if let Some(name) = input["name"].as_str() {
            let hits = self
                .workspace_store
                .search_nodes(tenant_id, user_id, name, 5)
                .await
                .map_err(|e| anyhow::anyhow!("search for '{name}': {e}"))?;
            let chosen = hits
                .iter()
                .find(|n| n.name.eq_ignore_ascii_case(name))
                .cloned()
                .or_else(|| hits.into_iter().next())
                .ok_or_else(|| anyhow::anyhow!("no workspace node matches name '{name}'"))?;
            chosen.id.to_string()
        } else {
            anyhow::bail!("move_node requires either `id` (or `node_id`) or `name`");
        };
        let ulid: ulid::Ulid = id_str
            .parse()
            .map_err(|e| anyhow::anyhow!("invalid node id '{id_str}': {e}"))?;
        let new_parent_id: Option<ulid::Ulid> = if let Some(s) = input["new_parent_id"]
            .as_str()
            .or_else(|| input["parent_id"].as_str())
        {
            if s.is_empty() {
                None
            } else {
                Some(
                    s.parse()
                        .map_err(|e| anyhow::anyhow!("invalid parent ulid '{s}': {e}"))?,
                )
            }
        } else if let Some(parent_name) = input["new_parent_name"]
            .as_str()
            .or_else(|| input["parent_name"].as_str())
        {
            let hits = self
                .workspace_store
                .search_nodes(tenant_id, user_id, parent_name, 5)
                .await
                .map_err(|e| anyhow::anyhow!("search for parent '{parent_name}': {e}"))?;
            let chosen = hits
                .iter()
                .find(|n| n.kind == NodeKind::Folder && n.name.eq_ignore_ascii_case(parent_name))
                .cloned()
                .or_else(|| hits.into_iter().find(|n| n.kind == NodeKind::Folder))
                .ok_or_else(|| anyhow::anyhow!("no folder matches parent name '{parent_name}'"))?;
            Some(chosen.id)
        } else {
            None
        };
        let moved = self
            .workspace_store
            .move_node(tenant_id, user_id, ulid, new_parent_id, None)
            .await
            .map_err(|e| anyhow::anyhow!("move_node {ulid}: {e}"))?;
        Ok(json!({
            "status": "moved",
            "id": moved.id.to_string(),
            "name": moved.name,
            "virtual_path": moved.virtual_path,
            "parent_id": moved.parent_id.map(|p| p.to_string()),
        }))
    }

    async fn delete_node(
        &self,
        input: &Value,
        tenant_id: &str,
        user_id: &str,
    ) -> anyhow::Result<Value> {
        let id_str = if let Some(s) = input["id"].as_str() {
            s.to_string()
        } else if let Some(name) = input["name"].as_str() {
            let hits = self
                .workspace_store
                .search_nodes(tenant_id, user_id, name, 5)
                .await
                .map_err(|e| anyhow::anyhow!("search for '{name}': {e}"))?;
            let exact = hits
                .iter()
                .find(|n| n.name.eq_ignore_ascii_case(name))
                .cloned();
            let chosen = exact
                .or_else(|| hits.into_iter().next())
                .ok_or_else(|| anyhow::anyhow!("no workspace node matches name '{name}'"))?;
            chosen.id.to_string()
        } else {
            anyhow::bail!("delete_node requires either `id` or `name`");
        };
        let ulid: ulid::Ulid = id_str
            .parse()
            .map_err(|e| anyhow::anyhow!("invalid node id '{id_str}': {e}"))?;
        self.workspace_store
            .delete_node(tenant_id, user_id, ulid)
            .await
            .map_err(|e| anyhow::anyhow!("delete_node {ulid}: {e}"))?;
        Ok(json!({ "status": "deleted", "id": id_str }))
    }

    async fn bulk_delete(
        &self,
        input: &Value,
        tenant_id: &str,
        user_id: &str,
    ) -> anyhow::Result<Value> {
        // Resolve the target parent — None means workspace root.
        let parent_id: Option<ulid::Ulid> = if let Some(s) = input["parent_id"]
            .as_str()
            .or_else(|| input["folder_id"].as_str())
        {
            Some(
                s.parse()
                    .map_err(|e| anyhow::anyhow!("invalid parent_id '{s}': {e}"))?,
            )
        } else if let Some(name) = input["parent_name"]
            .as_str()
            .or_else(|| input["folder_name"].as_str())
        {
            let hits = self
                .workspace_store
                .search_nodes(tenant_id, user_id, name, 5)
                .await
                .map_err(|e| anyhow::anyhow!("search for parent '{name}': {e}"))?;
            let chosen = hits
                .iter()
                .find(|n| n.kind == NodeKind::Folder && n.name.eq_ignore_ascii_case(name))
                .cloned()
                .or_else(|| hits.into_iter().find(|n| n.kind == NodeKind::Folder))
                .ok_or_else(|| anyhow::anyhow!("no folder matches name '{name}'"))?;
            Some(chosen.id)
        } else {
            // No parent specified → target the workspace root (all top-level items).
            None
        };

        let kind_filter = input["kind"].as_str().unwrap_or("all").to_ascii_lowercase();
        let children = self
            .workspace_store
            .list_accessible_children(tenant_id, user_id, parent_id)
            .await
            .unwrap_or_default();

        let mut deleted_ids: Vec<String> = Vec::new();
        let mut deleted_names: Vec<String> = Vec::new();
        let mut errors: Vec<Value> = Vec::new();
        let mut skipped: Vec<Value> = Vec::new();

        for child in children {
            let should_delete = match kind_filter.as_str() {
                "all" | "*" => true,
                "conversation" | "file" | "files" => child.kind == NodeKind::Conversation,
                "folder" | "folders" => child.kind == NodeKind::Folder,
                _ => true,
            };
            if !should_delete {
                skipped.push(json!({
                    "id": child.id.to_string(),
                    "name": child.name,
                    "reason": format!("kind {:?} excluded by filter '{}'", child.kind, kind_filter),
                }));
                continue;
            }
            match self
                .workspace_store
                .delete_node(tenant_id, user_id, child.id)
                .await
            {
                Ok(()) => {
                    deleted_ids.push(child.id.to_string());
                    deleted_names.push(child.name);
                }
                Err(e) => {
                    errors.push(json!({ "id": child.id.to_string(), "name": child.name, "error": e.to_string() }));
                }
            }
        }

        Ok(json!({
            "status": if errors.is_empty() { "ok" } else { "partial" },
            "scope": parent_id.map(|id| id.to_string()).unwrap_or_else(|| "<root>".into()),
            "kind_filter": kind_filter,
            "deleted_count": deleted_ids.len(),
            "deleted_names": deleted_names,
            "skipped_count": skipped.len(),
            "errors": errors,
        }))
    }
}

// ── NativeStorageFactory ──────────────────────────────────────────────────────

/// Factory for `ToolKind::Native` capabilities.
///
/// `storage-workspace` maps to `StorageWorkspaceProvider`; `[config] op = "plan"`
/// maps to `StaticPlanProvider` (returns `config.steps` for orchestration).
pub struct NativeStorageFactory {
    workspace_store: Arc<dyn WorkspaceStore>,
    workspace_content: Arc<dyn WorkspaceContentStore>,
}

impl NativeStorageFactory {
    pub fn new(
        workspace_store: Arc<dyn WorkspaceStore>,
        workspace_content: Arc<dyn WorkspaceContentStore>,
    ) -> Self {
        Self {
            workspace_store,
            workspace_content,
        }
    }
}

impl CapabilityFactory for NativeStorageFactory {
    fn supports(&self, kind: &ToolKind, _name: &str) -> bool {
        matches!(kind, ToolKind::Native)
    }

    fn create(&self, card: CapabilityCard) -> anyhow::Result<Arc<dyn CapabilityProvider>> {
        if card.manifest.name == "storage-workspace" {
            return Ok(Arc::new(StorageWorkspaceProvider::new(
                card.manifest,
                Arc::clone(&self.workspace_store),
                Arc::clone(&self.workspace_content),
            )));
        }

        let op = card.manifest.config["op"]
            .as_str()
            .unwrap_or("")
            .to_string();
        match op.as_str() {
            "plan" => Ok(Arc::new(StaticPlanProvider::new(card.manifest))),
            other => anyhow::bail!(
                "NativeStorageFactory: unknown op={other:?} for capability '{}'",
                card.manifest.name
            ),
        }
    }
}
