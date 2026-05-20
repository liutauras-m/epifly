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
        let full = safe_join(Path::new(&workspace_root), rel)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
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
        let full = safe_join(Path::new(&workspace_root), rel)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
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
        Self { manifest, workspace_store, workspace_content }
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
        let tenant = tenant
            .ok_or_else(|| anyhow::anyhow!("workspace tools require tenant context"))?;
        let tenant_id = tenant.tenant_id.as_str();
        let user_id = tenant.user_id.as_ref().map(|u| u.as_str()).unwrap_or("__dev__");

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
        let doc_name = if filename.ends_with(".md") {
            filename.to_string()
        } else {
            format!("{filename}.md")
        };

        let folders = self
            .workspace_store
            .list_accessible_children(tenant_id, user_id, None)
            .await
            .unwrap_or_default();

        let folder = if let Some(f) = folders.iter().find(|n| {
            n.kind == NodeKind::Folder && n.name.eq_ignore_ascii_case(folder_name)
        }) {
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
        let full = safe_join(Path::new(&workspace_root), rel)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
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
            safe_join(Path::new(&workspace_root), prefix)
                .map_err(|e| anyhow::anyhow!("{e}"))?
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
        let full = safe_join(Path::new(&workspace_root), rel)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
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
        let src = safe_join(Path::new(&workspace_root), from)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let dst = safe_join(Path::new(&workspace_root), to)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
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
        Self { manifest, steps_json }
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
pub struct NativeStorageFactory {
    workspace_store: Arc<dyn WorkspaceStore>,
    workspace_content: Arc<dyn WorkspaceContentStore>,
}

impl NativeStorageFactory {
    pub fn new(
        workspace_store: Arc<dyn WorkspaceStore>,
        workspace_content: Arc<dyn WorkspaceContentStore>,
    ) -> Self {
        Self { workspace_store, workspace_content }
    }
}

impl CapabilityFactory for NativeStorageFactory {
    fn supports(&self, kind: &ToolKind, _name: &str) -> bool {
        matches!(kind, ToolKind::Native)
    }

    fn create(&self, card: CapabilityCard) -> anyhow::Result<Arc<dyn CapabilityProvider>> {
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
