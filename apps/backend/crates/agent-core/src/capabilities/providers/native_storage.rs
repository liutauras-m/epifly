//! Native-storage capability providers — replace the old monolithic `BuiltinProvider`
//! and the gateway-level `WorkspaceProvider` with focused, independently testable units.
//!
//! Each provider handles exactly one storage concern; `NativeStorageFactory` dispatches
//! based on the `[config] op` field declared in each capability's TOML manifest.

use crate::capabilities::card::CapabilityCard;
use crate::capabilities::manifest::{ToolKind, ToolManifest};
use crate::capabilities::provider::{CapabilityFactory, CapabilityProvider};
use crate::context::tenant::TenantContext;
use async_trait::async_trait;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use chrono::Utc;
use common::memory::store::{WorkspaceContentStore, WorkspaceStore};
use common::memory::workspace::NodeKind;
use common::path_safety::safe_join;
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Arc;
use tracing::warn;

// ── ReadTextProvider ──────────────────────────────────────────────────────────

/// Reads a text file from the tenant's local workspace root (filesystem path).
pub struct ReadTextProvider {
    manifest: ToolManifest,
}

impl ReadTextProvider {
    pub fn new(manifest: ToolManifest) -> Self {
        Self { manifest }
    }
}

#[async_trait]
impl CapabilityProvider for ReadTextProvider {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        if tool_name != "read_file" && tool_name != "read" {
            anyhow::bail!("unknown tool: {tool_name}");
        }
        let workspace_root = workspace_root_for(tenant);
        let rel = input["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing required field: path"))?;
        let full =
            safe_join(Path::new(&workspace_root), rel).map_err(|e| anyhow::anyhow!("{e}"))?;
        let content = tokio::fs::read_to_string(&full)
            .await
            .map_err(|e| anyhow::anyhow!("read_file {rel}: {e}"))?;
        Ok(json!({ "path": rel, "content": content }))
    }
}

// ── WriteTextProvider ─────────────────────────────────────────────────────────

/// Writes text content to a file in the tenant's local workspace root.
pub struct WriteTextProvider {
    manifest: ToolManifest,
}

impl WriteTextProvider {
    pub fn new(manifest: ToolManifest) -> Self {
        Self { manifest }
    }
}

#[async_trait]
impl CapabilityProvider for WriteTextProvider {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        if tool_name != "write_file" && tool_name != "write" {
            anyhow::bail!("unknown tool: {tool_name}");
        }
        let workspace_root = workspace_root_for(tenant);
        let rel = input["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing required field: path"))?;
        let content = input["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing required field: content"))?;
        let full =
            safe_join(Path::new(&workspace_root), rel).map_err(|e| anyhow::anyhow!("{e}"))?;
        if let Some(parent) = full.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| anyhow::anyhow!("mkdir: {e}"))?;
        }
        tokio::fs::write(&full, content)
            .await
            .map_err(|e| anyhow::anyhow!("write_file {rel}: {e}"))?;
        Ok(json!({ "path": rel, "bytes_written": content.len() }))
    }
}

// ── WorkspaceNativeProvider ───────────────────────────────────────────────────

/// Implements `save_document` and `list_folders` against the in-process WorkspaceStore.
/// Replaces the gateway-side `WorkspaceProvider` so the capability is registered via TOML.
pub struct WorkspaceNativeProvider {
    manifest: ToolManifest,
    workspace_store: Arc<dyn WorkspaceStore>,
    workspace_content: Arc<dyn WorkspaceContentStore>,
}

impl WorkspaceNativeProvider {
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
impl CapabilityProvider for WorkspaceNativeProvider {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        let tenant =
            tenant.ok_or_else(|| anyhow::anyhow!("workspace tools require tenant context"))?;
        let tenant_id = tenant.tenant_id.as_str();
        let user_id = tenant
            .user_id
            .as_ref()
            .map(|u| u.as_str())
            .unwrap_or("__dev__");

        match tool_name {
            "save_document" => self.save_document(input, tenant_id, user_id).await,
            "list_folders" => self.list_folders(tenant_id, user_id).await,
            other => anyhow::bail!("unknown workspace tool: {other}"),
        }
    }
}

impl WorkspaceNativeProvider {
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
}

// ── PutObjectProvider ─────────────────────────────────────────────────────────

/// Writes a binary file (base64-encoded content) to the tenant's workspace.
pub struct PutObjectProvider {
    manifest: ToolManifest,
}

impl PutObjectProvider {
    pub fn new(manifest: ToolManifest) -> Self {
        Self { manifest }
    }
}

#[async_trait]
impl CapabilityProvider for PutObjectProvider {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        if tool_name != "put_object" && tool_name != "put" {
            anyhow::bail!("unknown tool: {tool_name}");
        }
        let workspace_root = workspace_root_for(tenant);
        let rel = input["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing required field: path"))?;
        let full =
            safe_join(Path::new(&workspace_root), rel).map_err(|e| anyhow::anyhow!("{e}"))?;
        if let Some(parent) = full.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| anyhow::anyhow!("mkdir: {e}"))?;
        }
        let bytes = if let Some(b64) = input["content_base64"].as_str() {
            BASE64
                .decode(b64)
                .map_err(|e| anyhow::anyhow!("invalid base64: {e}"))?
        } else if let Some(text) = input["content"].as_str() {
            text.as_bytes().to_vec()
        } else {
            anyhow::bail!("missing required field: content or content_base64");
        };
        let n = bytes.len();
        tokio::fs::write(&full, bytes)
            .await
            .map_err(|e| anyhow::anyhow!("put_object {rel}: {e}"))?;
        Ok(json!({ "path": rel, "bytes_written": n }))
    }
}

// ── ListDirsProvider ──────────────────────────────────────────────────────────

/// Lists subdirectories (and optionally files) under a prefix in the workspace.
pub struct ListFoldersProvider {
    manifest: ToolManifest,
}

impl ListFoldersProvider {
    pub fn new(manifest: ToolManifest) -> Self {
        Self { manifest }
    }
}

#[async_trait]
impl CapabilityProvider for ListFoldersProvider {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        if tool_name != "list_folders" && tool_name != "list_dirs" && tool_name != "list" {
            anyhow::bail!("unknown tool: {tool_name}");
        }
        let workspace_root = workspace_root_for(tenant);
        let prefix = input["prefix"].as_str().unwrap_or(".");
        let base = if prefix == "." {
            Path::new(&workspace_root).to_path_buf()
        } else {
            safe_join(Path::new(&workspace_root), prefix).map_err(|e| anyhow::anyhow!("{e}"))?
        };
        let mut entries = Vec::new();
        let mut rd = tokio::fs::read_dir(&base)
            .await
            .map_err(|e| anyhow::anyhow!("list_dirs {prefix}: {e}"))?;
        while let Some(entry) = rd.next_entry().await? {
            let ft = entry.file_type().await?;
            entries.push(json!({
                "name": entry.file_name().to_string_lossy(),
                "kind": if ft.is_dir() { "dir" } else { "file" },
            }));
        }
        Ok(json!({ "prefix": prefix, "entries": entries, "count": entries.len() }))
    }
}

// ── EnsureFolderProvider ──────────────────────────────────────────────────────

/// Creates a directory (including all parents) if it does not already exist.
pub struct EnsureFolderProvider {
    manifest: ToolManifest,
}

impl EnsureFolderProvider {
    pub fn new(manifest: ToolManifest) -> Self {
        Self { manifest }
    }
}

#[async_trait]
impl CapabilityProvider for EnsureFolderProvider {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        if tool_name != "ensure_folder" && tool_name != "ensure" {
            anyhow::bail!("unknown tool: {tool_name}");
        }
        let workspace_root = workspace_root_for(tenant);
        let rel = input["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing required field: path"))?;
        let full =
            safe_join(Path::new(&workspace_root), rel).map_err(|e| anyhow::anyhow!("{e}"))?;
        tokio::fs::create_dir_all(&full)
            .await
            .map_err(|e| anyhow::anyhow!("ensure_folder {rel}: {e}"))?;
        Ok(json!({ "path": rel, "created": true }))
    }
}

// ── EnsureDateFolderProvider ──────────────────────────────────────────────────

/// Creates a date-partitioned directory `{base}/{YYYY}/{MM}/{DD}/` for today's date.
pub struct EnsureDateFolderProvider {
    manifest: ToolManifest,
}

impl EnsureDateFolderProvider {
    pub fn new(manifest: ToolManifest) -> Self {
        Self { manifest }
    }
}

#[async_trait]
impl CapabilityProvider for EnsureDateFolderProvider {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        if tool_name != "ensure_date_folder" {
            anyhow::bail!("unknown tool: {tool_name}");
        }
        let workspace_root = workspace_root_for(tenant);
        let base = input["base_path"].as_str().unwrap_or("uploads");
        let now = Utc::now();
        let date_path = format!("{}/{}", base, now.format("%Y/%m/%d"));
        let full = safe_join(Path::new(&workspace_root), &date_path)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        tokio::fs::create_dir_all(&full)
            .await
            .map_err(|e| anyhow::anyhow!("ensure_date_folder {date_path}: {e}"))?;
        Ok(json!({ "path": date_path, "created": true }))
    }
}

// ── MoveObjectProvider ────────────────────────────────────────────────────────

/// Moves (renames) a file within the tenant workspace.
pub struct MoveObjectProvider {
    manifest: ToolManifest,
}

impl MoveObjectProvider {
    pub fn new(manifest: ToolManifest) -> Self {
        Self { manifest }
    }
}

#[async_trait]
impl CapabilityProvider for MoveObjectProvider {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        if tool_name != "move_object" && tool_name != "move" {
            anyhow::bail!("unknown tool: {tool_name}");
        }
        let workspace_root = workspace_root_for(tenant);
        let from = input["from"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing required field: from"))?;
        let to = input["to"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing required field: to"))?;
        let src =
            safe_join(Path::new(&workspace_root), from).map_err(|e| anyhow::anyhow!("{e}"))?;
        let dst = safe_join(Path::new(&workspace_root), to).map_err(|e| anyhow::anyhow!("{e}"))?;
        if let Some(parent) = dst.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| anyhow::anyhow!("mkdir for move destination: {e}"))?;
        }
        tokio::fs::rename(&src, &dst)
            .await
            .map_err(|e| anyhow::anyhow!("move_object {from} → {to}: {e}"))?;
        Ok(json!({ "from": from, "to": to, "moved": true }))
    }
}

// ── TagObjectProvider ─────────────────────────────────────────────────────────

/// Writes key-value metadata tags to a `.{filename}.meta.json` sidecar file.
pub struct TagObjectProvider {
    manifest: ToolManifest,
}

impl TagObjectProvider {
    pub fn new(manifest: ToolManifest) -> Self {
        Self { manifest }
    }
}

#[async_trait]
impl CapabilityProvider for TagObjectProvider {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        if tool_name != "tag_object" && tool_name != "tag" {
            anyhow::bail!("unknown tool: {tool_name}");
        }
        let workspace_root = workspace_root_for(tenant);
        let rel = input["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing required field: path"))?;
        let tags = input.get("tags").cloned().unwrap_or(json!({}));
        // Derive sidecar path: `dir/.filename.meta.json`
        let sidecar_rel = {
            let p = std::path::Path::new(rel);
            let fname = p
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "file".into());
            let dir = p.parent().and_then(|d| d.to_str()).unwrap_or(".");
            if dir == "." {
                format!(".{fname}.meta.json")
            } else {
                format!("{dir}/.{fname}.meta.json")
            }
        };
        let full = safe_join(Path::new(&workspace_root), &sidecar_rel)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        if let Some(parent) = full.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let json_str = serde_json::to_string_pretty(&tags)?;
        tokio::fs::write(&full, json_str)
            .await
            .map_err(|e| anyhow::anyhow!("tag_object {rel}: {e}"))?;
        Ok(json!({ "path": rel, "sidecar": sidecar_rel, "tagged": true }))
    }
}

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

// ── MoveNodeProvider ──────────────────────────────────────────────────────────

/// Moves a workspace node (folder or conversation) to a new parent in the
/// workspace tree. Distinct from `MoveObjectProvider`, which moves files on
/// disk via `tokio::fs::rename`. This provider operates on the
/// `WorkspaceStore` so the UI tree reflects the change.
pub struct MoveNodeProvider {
    manifest: ToolManifest,
    workspace_store: Arc<dyn WorkspaceStore>,
}

impl MoveNodeProvider {
    pub fn new(manifest: ToolManifest, workspace_store: Arc<dyn WorkspaceStore>) -> Self {
        Self {
            manifest,
            workspace_store,
        }
    }
}

#[async_trait]
impl CapabilityProvider for MoveNodeProvider {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        if tool_name != "move_node" && tool_name != "relocate" {
            anyhow::bail!("unknown tool: {tool_name}");
        }
        let tenant = tenant.ok_or_else(|| anyhow::anyhow!("move_node requires tenant context"))?;
        let tenant_id = tenant.tenant_id.as_str();
        let user_id = tenant
            .user_id
            .as_ref()
            .map(|u| u.as_str())
            .unwrap_or("__dev__");

        // Resolve target node: prefer explicit `id`, fall back to `name` lookup.
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

        // Resolve new parent: explicit ULID, parent name lookup, or null (move to root).
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
}

// ── CreateFolderProvider ──────────────────────────────────────────────────────

/// Creates an empty folder in the tenant's workspace tree. Unlike
/// `EnsureFolderProvider` (filesystem mkdir), this writes to the
/// `WorkspaceStore` so the folder shows up in the UI tree and the
/// `/v1/workspaces/tree` endpoint.
pub struct CreateFolderProvider {
    manifest: ToolManifest,
    workspace_store: Arc<dyn WorkspaceStore>,
}

impl CreateFolderProvider {
    pub fn new(manifest: ToolManifest, workspace_store: Arc<dyn WorkspaceStore>) -> Self {
        Self {
            manifest,
            workspace_store,
        }
    }
}

#[async_trait]
impl CapabilityProvider for CreateFolderProvider {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        if tool_name != "create_folder" && tool_name != "new_folder" {
            anyhow::bail!("unknown tool: {tool_name}");
        }
        let tenant =
            tenant.ok_or_else(|| anyhow::anyhow!("create_folder requires tenant context"))?;
        let tenant_id = tenant.tenant_id.as_str();
        let user_id = tenant
            .user_id
            .as_ref()
            .map(|u| u.as_str())
            .unwrap_or("__dev__");

        let name = input["name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing required field: name"))?
            .trim();
        if name.is_empty() {
            anyhow::bail!("name must not be empty");
        }

        // Optional parent — accept ULID or null to mean "at the root".
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
}

// ── DeleteNodeProvider ────────────────────────────────────────────────────────

/// Deletes a workspace node (folder or conversation) by ULID or by name.
/// When `name` is supplied, the provider first searches the user's accessible
/// nodes and resolves the ULID before issuing the delete. This is the path
/// the LLM uses — it doesn't have a human-name → ULID lookup otherwise.
pub struct DeleteNodeProvider {
    manifest: ToolManifest,
    workspace_store: Arc<dyn WorkspaceStore>,
}

impl DeleteNodeProvider {
    pub fn new(manifest: ToolManifest, workspace_store: Arc<dyn WorkspaceStore>) -> Self {
        Self {
            manifest,
            workspace_store,
        }
    }
}

#[async_trait]
impl CapabilityProvider for DeleteNodeProvider {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        if tool_name != "delete_node" && tool_name != "delete" && tool_name != "remove" {
            anyhow::bail!("unknown tool: {tool_name}");
        }
        let tenant =
            tenant.ok_or_else(|| anyhow::anyhow!("delete_node requires tenant context"))?;
        let tenant_id = tenant.tenant_id.as_str();
        let user_id = tenant
            .user_id
            .as_ref()
            .map(|u| u.as_str())
            .unwrap_or("__dev__");

        // Resolve target ULID: prefer explicit `id`, fall back to `name` lookup.
        let id_str = if let Some(s) = input["id"].as_str() {
            s.to_string()
        } else if let Some(name) = input["name"].as_str() {
            let hits = self
                .workspace_store
                .search_nodes(tenant_id, user_id, name, 5)
                .await
                .map_err(|e| anyhow::anyhow!("search for '{name}': {e}"))?;
            // Prefer an exact case-insensitive name match; fall back to top hit.
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

        Ok(json!({
            "status": "deleted",
            "id": id_str,
        }))
    }
}

// ── FindByNameProvider ────────────────────────────────────────────────────────

/// Resolves a human-readable name to a workspace node ULID. Returns the top
/// match (exact name preferred, then the highest-ranked hit from
/// `search_nodes`). Used by the agent to chain "find X" → "move/delete X".
pub struct FindByNameProvider {
    manifest: ToolManifest,
    workspace_store: Arc<dyn WorkspaceStore>,
}

impl FindByNameProvider {
    pub fn new(manifest: ToolManifest, workspace_store: Arc<dyn WorkspaceStore>) -> Self {
        Self {
            manifest,
            workspace_store,
        }
    }
}

#[async_trait]
impl CapabilityProvider for FindByNameProvider {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        if tool_name != "find_by_name" && tool_name != "find" && tool_name != "lookup" {
            anyhow::bail!("unknown tool: {tool_name}");
        }
        let tenant =
            tenant.ok_or_else(|| anyhow::anyhow!("find_by_name requires tenant context"))?;
        let tenant_id = tenant.tenant_id.as_str();
        let user_id = tenant
            .user_id
            .as_ref()
            .map(|u| u.as_str())
            .unwrap_or("__dev__");

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

        // Score exact name matches highest.
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
}

// ── ShowTreeProvider ──────────────────────────────────────────────────────────

/// Returns a formatted Markdown tree of the user's accessible workspace nodes.
/// Optionally scoped to a `parent_id`; otherwise lists from the root.
pub struct ShowTreeProvider {
    manifest: ToolManifest,
    workspace_store: Arc<dyn WorkspaceStore>,
}

impl ShowTreeProvider {
    pub fn new(manifest: ToolManifest, workspace_store: Arc<dyn WorkspaceStore>) -> Self {
        Self {
            manifest,
            workspace_store,
        }
    }
}

#[async_trait]
impl CapabilityProvider for ShowTreeProvider {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        if tool_name != "show_tree" && tool_name != "tree" && tool_name != "show" {
            anyhow::bail!("unknown tool: {tool_name}");
        }
        let tenant = tenant.ok_or_else(|| anyhow::anyhow!("show_tree requires tenant context"))?;
        let tenant_id = tenant.tenant_id.as_str();
        let user_id = tenant
            .user_id
            .as_ref()
            .map(|u| u.as_str())
            .unwrap_or("__dev__");

        let root_id: Option<ulid::Ulid> = input["parent_id"].as_str().and_then(|s| s.parse().ok());
        // Depth cap prevents unbounded recursion on pathological trees.
        let max_depth = input["depth"].as_u64().unwrap_or(2).min(5) as usize;

        let mut buf = String::new();
        let mut count = 0_usize;
        self.render_subtree(
            tenant_id, user_id, root_id, 0, max_depth, &mut buf, &mut count,
        )
        .await?;

        if buf.is_empty() {
            buf.push_str("_(workspace is empty)_");
        }

        Ok(json!({
            "tree": buf,
            "node_count": count,
        }))
    }
}

impl ShowTreeProvider {
    #[allow(clippy::too_many_arguments)]
    async fn render_subtree(
        &self,
        tenant_id: &str,
        user_id: &str,
        parent: Option<ulid::Ulid>,
        depth: usize,
        max_depth: usize,
        out: &mut String,
        count: &mut usize,
    ) -> anyhow::Result<()> {
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
    }
}

// ── BulkDeleteProvider ────────────────────────────────────────────────────────

/// Deletes every direct (or recursive) child of a workspace folder in one tool
/// call. Used for "delete all files in X" prompts so the agent doesn't have to
/// chain N `delete_node` calls and run out of model rounds.
///
/// Safety: requires either `parent_id` or `parent_name` — there is no "delete
/// everything in the workspace" mode. The caller can pass `kind: "conversation"`
/// to limit the operation to files only and preserve sub-folders, or `kind: "all"`
/// to wipe everything beneath the target.
pub struct BulkDeleteProvider {
    manifest: ToolManifest,
    workspace_store: Arc<dyn WorkspaceStore>,
}

impl BulkDeleteProvider {
    pub fn new(manifest: ToolManifest, workspace_store: Arc<dyn WorkspaceStore>) -> Self {
        Self {
            manifest,
            workspace_store,
        }
    }
}

#[async_trait]
impl CapabilityProvider for BulkDeleteProvider {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        if tool_name != "bulk_delete" && tool_name != "delete_all" && tool_name != "empty_folder" {
            anyhow::bail!("unknown tool: {tool_name}");
        }
        let tenant =
            tenant.ok_or_else(|| anyhow::anyhow!("bulk_delete requires tenant context"))?;
        let tenant_id = tenant.tenant_id.as_str();
        let user_id = tenant
            .user_id
            .as_ref()
            .map(|u| u.as_str())
            .unwrap_or("__dev__");

        // Resolve parent folder: explicit ULID or name lookup.
        let parent_id: ulid::Ulid = if let Some(s) = input["parent_id"]
            .as_str()
            .or_else(|| input["folder_id"].as_str())
        {
            s.parse()
                .map_err(|e| anyhow::anyhow!("invalid parent_id '{s}': {e}"))?
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
            chosen.id
        } else {
            anyhow::bail!("bulk_delete requires either `parent_id` or `parent_name`");
        };

        // Filter scope: "all" (default), "conversation" (files only), "folder".
        let kind_filter = input["kind"].as_str().unwrap_or("all").to_ascii_lowercase();

        let children = self
            .workspace_store
            .list_accessible_children(tenant_id, user_id, Some(parent_id))
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
                    errors.push(json!({
                        "id": child.id.to_string(),
                        "name": child.name,
                        "error": e.to_string(),
                    }));
                }
            }
        }

        Ok(json!({
            "status": if errors.is_empty() { "ok" } else { "partial" },
            "parent_id": parent_id.to_string(),
            "kind_filter": kind_filter,
            "deleted_count": deleted_ids.len(),
            "deleted_ids": deleted_ids,
            "deleted_names": deleted_names,
            "skipped": skipped,
            "errors": errors,
        }))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CONSOLIDATED DOMAIN PROVIDERS (Phase 1 — capabilities consolidation refactor)
// StorageWorkspaceProvider: 11 workspace-tree tools in one card.
// StorageFsProvider:        5 filesystem-path tools in one card.
// Legacy per-op providers above remain as rollback safety (removed in Phase 7).
// ═══════════════════════════════════════════════════════════════════════════════

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
        // Filesystem-backed tools don't require tenant context (workspace_root has a fallback).
        if matches!(
            tool_name,
            "ensure_folder" | "ensure_date_folder" | "tag_object"
        ) {
            let root = workspace_root_for(tenant);
            return match tool_name {
                "ensure_folder" => self.ensure_folder(input, &root).await,
                "ensure_date_folder" => self.ensure_date_folder(input, &root).await,
                "tag_object" => self.tag_object(input, &root).await,
                _ => unreachable!(),
            };
        }

        let t = tenant.ok_or_else(|| anyhow::anyhow!("workspace tools require tenant context"))?;
        let tenant_id = t.tenant_id.as_str();
        let user_id = t.user_id.as_ref().map(|u| u.as_str()).unwrap_or("__dev__");

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

    async fn ensure_folder(&self, input: &Value, workspace_root: &str) -> anyhow::Result<Value> {
        let rel = input["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing required field: path"))?;
        let full = safe_join(Path::new(workspace_root), rel).map_err(|e| anyhow::anyhow!("{e}"))?;
        tokio::fs::create_dir_all(&full)
            .await
            .map_err(|e| anyhow::anyhow!("ensure_folder {rel}: {e}"))?;
        Ok(json!({ "path": rel, "created": true }))
    }

    async fn ensure_date_folder(
        &self,
        input: &Value,
        workspace_root: &str,
    ) -> anyhow::Result<Value> {
        let base = input["base_path"].as_str().unwrap_or("uploads");
        let now = Utc::now();
        let date_path = format!("{}/{}", base, now.format("%Y/%m/%d"));
        let full =
            safe_join(Path::new(workspace_root), &date_path).map_err(|e| anyhow::anyhow!("{e}"))?;
        tokio::fs::create_dir_all(&full)
            .await
            .map_err(|e| anyhow::anyhow!("ensure_date_folder {date_path}: {e}"))?;
        Ok(json!({ "path": date_path, "created": true }))
    }

    async fn tag_object(&self, input: &Value, workspace_root: &str) -> anyhow::Result<Value> {
        let rel = input["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing required field: path"))?;
        let tags = input.get("tags").cloned().unwrap_or(json!({}));
        let sidecar_rel = {
            let p = std::path::Path::new(rel);
            let fname = p
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "file".into());
            let dir = p.parent().and_then(|d| d.to_str()).unwrap_or(".");
            if dir == "." {
                format!(".{fname}.meta.json")
            } else {
                format!("{dir}/.{fname}.meta.json")
            }
        };
        let full = safe_join(Path::new(workspace_root), &sidecar_rel)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        if let Some(parent) = full.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let json_str = serde_json::to_string_pretty(&tags)?;
        tokio::fs::write(&full, json_str)
            .await
            .map_err(|e| anyhow::anyhow!("tag_object {rel}: {e}"))?;
        Ok(json!({ "path": rel, "sidecar": sidecar_rel, "tagged": true }))
    }
}

// ── StorageFsProvider ─────────────────────────────────────────────────────────

/// Low-level filesystem toolkit — 5 tools operating on explicit paths.
/// `list_paths` is the renamed successor to the legacy `list_folders` fs variant.
pub struct StorageFsProvider {
    manifest: ToolManifest,
}

impl StorageFsProvider {
    pub fn new(manifest: ToolManifest) -> Self {
        Self { manifest }
    }
}

#[async_trait]
impl CapabilityProvider for StorageFsProvider {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        let root = workspace_root_for(tenant);
        match tool_name {
            "read_file" => self.read_file(input, &root).await,
            "write_file" => self.write_file(input, &root).await,
            "put_object" => self.put_object(input, &root).await,
            "move_object" => self.move_object(input, &root).await,
            "list_paths" => self.list_paths(input, &root).await,
            other => anyhow::bail!("unknown tool '{other}' for storage-fs"),
        }
    }
}

impl StorageFsProvider {
    async fn read_file(&self, input: &Value, workspace_root: &str) -> anyhow::Result<Value> {
        let rel = input["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing required field: path"))?;
        let full = safe_join(Path::new(workspace_root), rel).map_err(|e| anyhow::anyhow!("{e}"))?;
        let content = tokio::fs::read_to_string(&full)
            .await
            .map_err(|e| anyhow::anyhow!("read_file {rel}: {e}"))?;
        Ok(json!({ "path": rel, "content": content }))
    }

    async fn write_file(&self, input: &Value, workspace_root: &str) -> anyhow::Result<Value> {
        let rel = input["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing required field: path"))?;
        let content = input["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing required field: content"))?;
        let full = safe_join(Path::new(workspace_root), rel).map_err(|e| anyhow::anyhow!("{e}"))?;
        if let Some(parent) = full.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| anyhow::anyhow!("mkdir: {e}"))?;
        }
        tokio::fs::write(&full, content)
            .await
            .map_err(|e| anyhow::anyhow!("write_file {rel}: {e}"))?;
        Ok(json!({ "path": rel, "bytes_written": content.len() }))
    }

    async fn put_object(&self, input: &Value, workspace_root: &str) -> anyhow::Result<Value> {
        let rel = input["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing required field: path"))?;
        let full = safe_join(Path::new(workspace_root), rel).map_err(|e| anyhow::anyhow!("{e}"))?;
        if let Some(parent) = full.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| anyhow::anyhow!("mkdir: {e}"))?;
        }
        let bytes = if let Some(b64) = input["content_base64"].as_str() {
            BASE64
                .decode(b64)
                .map_err(|e| anyhow::anyhow!("invalid base64: {e}"))?
        } else if let Some(text) = input["content"].as_str() {
            text.as_bytes().to_vec()
        } else {
            anyhow::bail!("missing required field: content or content_base64");
        };
        let n = bytes.len();
        tokio::fs::write(&full, bytes)
            .await
            .map_err(|e| anyhow::anyhow!("put_object {rel}: {e}"))?;
        Ok(json!({ "path": rel, "bytes_written": n }))
    }

    async fn move_object(&self, input: &Value, workspace_root: &str) -> anyhow::Result<Value> {
        let from = input["from"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing required field: from"))?;
        let to = input["to"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing required field: to"))?;
        let src = safe_join(Path::new(workspace_root), from).map_err(|e| anyhow::anyhow!("{e}"))?;
        let dst = safe_join(Path::new(workspace_root), to).map_err(|e| anyhow::anyhow!("{e}"))?;
        if let Some(parent) = dst.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| anyhow::anyhow!("mkdir for move destination: {e}"))?;
        }
        tokio::fs::rename(&src, &dst)
            .await
            .map_err(|e| anyhow::anyhow!("move_object {from} → {to}: {e}"))?;
        Ok(json!({ "from": from, "to": to, "moved": true }))
    }

    async fn list_paths(&self, input: &Value, workspace_root: &str) -> anyhow::Result<Value> {
        let prefix = input["prefix"].as_str().unwrap_or(".");
        let base = if prefix == "." {
            Path::new(workspace_root).to_path_buf()
        } else {
            safe_join(Path::new(workspace_root), prefix).map_err(|e| anyhow::anyhow!("{e}"))?
        };
        let mut entries = Vec::new();
        let mut rd = tokio::fs::read_dir(&base)
            .await
            .map_err(|e| anyhow::anyhow!("list_paths {prefix}: {e}"))?;
        while let Some(entry) = rd.next_entry().await? {
            let ft = entry.file_type().await?;
            entries.push(json!({
                "name": entry.file_name().to_string_lossy(),
                "kind": if ft.is_dir() { "dir" } else { "file" },
            }));
        }
        Ok(json!({ "prefix": prefix, "entries": entries, "count": entries.len() }))
    }
}

// ── NativeStorageFactory ──────────────────────────────────────────────────────

/// Factory for `ToolKind::Native` capabilities.
///
/// Dispatches to the correct provider based on the `[config] op` field in the manifest TOML:
/// - `"workspace"` → `WorkspaceNativeProvider` (save_document + list_folders)
/// - `"read_text"` → `ReadTextProvider` (filesystem read_file)
/// - `"write_text"` → `WriteTextProvider` (filesystem write_file)
/// - `"put_object"` → `PutObjectProvider` (binary/text put to workspace path)
/// - `"list_folders"` → `ListFoldersProvider` (list filesystem entries under a prefix)
/// - `"ensure_folder"` → `EnsureFolderProvider` (mkdir -p)
/// - `"ensure_date_folder"` → `EnsureDateFolderProvider` (dated partition directory)
/// - `"move_object"` → `MoveObjectProvider` (rename within workspace)
/// - `"tag_object"` → `TagObjectProvider` (write `.meta.json` sidecar)
/// - `"plan"` → `StaticPlanProvider` (returns `config.steps` array for orchestration)
/// - `"delete_node"` → `DeleteNodeProvider` (delete a workspace node by id or name)
/// - `"find_by_name"` → `FindByNameProvider` (resolve a name to a ULID + matches)
/// - `"show_tree"` → `ShowTreeProvider` (Markdown tree of workspace nodes)
/// - `"create_folder"` → `CreateFolderProvider` (empty workspace folder)
/// - `"move_node"` → `MoveNodeProvider` (move a workspace tree node by id or name)
/// - `"bulk_delete"` → `BulkDeleteProvider` (delete every child of a folder in one call)
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
        // Dispatch consolidated domain providers by manifest name first.
        match card.manifest.name.as_str() {
            "storage-workspace" => {
                return Ok(Arc::new(StorageWorkspaceProvider::new(
                    card.manifest,
                    Arc::clone(&self.workspace_store),
                    Arc::clone(&self.workspace_content),
                )));
            }
            "storage-fs" => return Ok(Arc::new(StorageFsProvider::new(card.manifest))),
            _ => {}
        }

        // Legacy single-op dispatch (rollback safety net for the migration window).
        let op = card.manifest.config["op"]
            .as_str()
            .unwrap_or("")
            .to_string();
        match op.as_str() {
            "workspace" => Ok(Arc::new(WorkspaceNativeProvider::new(
                card.manifest,
                Arc::clone(&self.workspace_store),
                Arc::clone(&self.workspace_content),
            ))),
            "read_text" => Ok(Arc::new(ReadTextProvider::new(card.manifest))),
            "write_text" => Ok(Arc::new(WriteTextProvider::new(card.manifest))),
            "put_object" => Ok(Arc::new(PutObjectProvider::new(card.manifest))),
            "list_folders" | "list_dirs" => Ok(Arc::new(ListFoldersProvider::new(card.manifest))),
            "ensure_folder" => Ok(Arc::new(EnsureFolderProvider::new(card.manifest))),
            "ensure_date_folder" => Ok(Arc::new(EnsureDateFolderProvider::new(card.manifest))),
            "move_object" => Ok(Arc::new(MoveObjectProvider::new(card.manifest))),
            "tag_object" => Ok(Arc::new(TagObjectProvider::new(card.manifest))),
            "plan" => Ok(Arc::new(StaticPlanProvider::new(card.manifest))),
            "delete_node" | "delete" | "remove" => Ok(Arc::new(DeleteNodeProvider::new(
                card.manifest,
                Arc::clone(&self.workspace_store),
            ))),
            "find_by_name" | "find" | "lookup" => Ok(Arc::new(FindByNameProvider::new(
                card.manifest,
                Arc::clone(&self.workspace_store),
            ))),
            "show_tree" | "tree" | "show" => Ok(Arc::new(ShowTreeProvider::new(
                card.manifest,
                Arc::clone(&self.workspace_store),
            ))),
            "create_folder" | "new_folder" => Ok(Arc::new(CreateFolderProvider::new(
                card.manifest,
                Arc::clone(&self.workspace_store),
            ))),
            "move_node" | "relocate" => Ok(Arc::new(MoveNodeProvider::new(
                card.manifest,
                Arc::clone(&self.workspace_store),
            ))),
            "bulk_delete" | "delete_all" | "empty_folder" => Ok(Arc::new(BulkDeleteProvider::new(
                card.manifest,
                Arc::clone(&self.workspace_store),
            ))),
            other => anyhow::bail!(
                "NativeStorageFactory: unknown op={other:?} for capability '{}'",
                card.manifest.name
            ),
        }
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn workspace_root_for(tenant: Option<&TenantContext>) -> String {
    tenant
        .map(|t| t.workspace_root.to_string_lossy().to_string())
        .unwrap_or_else(|| {
            std::env::var("CONUSAI_WORKSPACE_ROOT")
                .unwrap_or_else(|_| "/tmp/conusai/workspaces".into())
        })
}
