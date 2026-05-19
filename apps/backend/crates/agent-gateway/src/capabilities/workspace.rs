//! Built-in workspace capability — lets the agent save documents and list folders.
//!
//! Gateway routes inject `Arc<dyn WorkspaceStorage>` (the narrow Phase-1 trait) so
//! capabilities cannot access multipart, staging, or raw credentials.
//!
//! # Tools
//! - `workspace__save_document` — create a markdown document under a workspace folder
//! - `workspace__list_folders`  — list top-level workspace folders

use agent_core::capabilities::card::CapabilityCard;
use agent_core::capabilities::manifest::{ToolDef, ToolKind, ToolManifest};
use agent_core::capabilities::provider::CapabilityProvider;
use agent_core::context::tenant::TenantContext;
use agent_core::{VirtualPath, WorkspaceStorage};
use async_trait::async_trait;
use bytes::Bytes;
use common::memory::store::{WorkspaceContentStore, WorkspaceStore};
use common::memory::workspace::NodeKind;
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::warn;

pub struct WorkspaceProvider {
    manifest: ToolManifest,
    workspace_store: Arc<dyn WorkspaceStore>,
    workspace_content: Arc<dyn WorkspaceContentStore>,
    /// Narrow capability-safe storage interface. When provided, `save_document`
    /// uses it instead of the legacy `WorkspaceContentStore`.
    workspace_storage: Option<Arc<dyn WorkspaceStorage>>,
}

impl WorkspaceProvider {
    pub fn new(
        workspace_store: Arc<dyn WorkspaceStore>,
        workspace_content: Arc<dyn WorkspaceContentStore>,
    ) -> Self {
        Self::with_storage(workspace_store, workspace_content, None)
    }

    pub fn with_storage(
        workspace_store: Arc<dyn WorkspaceStore>,
        workspace_content: Arc<dyn WorkspaceContentStore>,
        workspace_storage: Option<Arc<dyn WorkspaceStorage>>,
    ) -> Self {
        let manifest = ToolManifest {
            name: "workspace".into(),
            version: "1.0.0".into(),
            description: "Save documents and list folders in the user's workspace".into(),
            kind: ToolKind::Native,
            config: Value::Null,
            tags: vec!["workspace".into(), "files".into(), "storage".into()],
            namespace: None,
            tools: vec![
                ToolDef {
                    name: "save_document".into(),
                    description: "Save text content as a document in a workspace folder. \
                        Creates the folder if it doesn't exist. \
                        Use this to save files, notes, or any content the user asks to store."
                        .into(),
                    input_schema: json!({
                        "type": "object",
                        "properties": {
                            "folder_name": {
                                "type": "string",
                                "description": "Name of the top-level workspace folder to save into (e.g. 'Research', 'Projects')"
                            },
                            "filename": {
                                "type": "string",
                                "description": "Document filename without extension (e.g. 'meeting-notes'). A .md extension will be added."
                            },
                            "content": {
                                "type": "string",
                                "description": "Markdown content to save in the document"
                            }
                        },
                        "required": ["folder_name", "filename", "content"]
                    }),
                },
                ToolDef {
                    name: "list_folders".into(),
                    description: "List all top-level workspace folders available to the user".into(),
                    input_schema: json!({
                        "type": "object",
                        "properties": {}
                    }),
                },
            ],
            chain: None,
            tenant_scope: vec![],
            enabled: true,
            search_keywords: vec![
                "save".into(),
                "store".into(),
                "folder".into(),
                "workspace".into(),
                "document".into(),
                "file".into(),
            ],
        };
        Self { manifest, workspace_store, workspace_content, workspace_storage }
    }

    pub fn into_card(self) -> CapabilityCard {
        let provider: Arc<dyn CapabilityProvider> = Arc::new(self);
        let card = CapabilityCard::new(
            provider.manifest().clone(),
            std::path::PathBuf::from("."),
        );
        card.with_provider(provider)
    }
}

#[async_trait]
impl CapabilityProvider for WorkspaceProvider {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn invoke(
        &self,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        let tenant = tenant.ok_or_else(|| anyhow::anyhow!("workspace tools require tenant context"))?;
        let tenant_id = tenant.tenant_id.as_str();
        let user_id = tenant.user_id.as_ref().map(|u| u.as_str()).unwrap_or("__dev__");

        match tool_name {
            "save_document" => self.save_document(input, tenant_id, user_id).await,
            "list_folders" => self.list_folders(tenant_id, user_id).await,
            other => anyhow::bail!("unknown workspace tool: {other}"),
        }
    }
}

impl WorkspaceProvider {
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

        // Find or create the top-level folder.
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

        // Write content — prefer the narrow WorkspaceStorage trait if available.
        let write_result = if let Some(ref ws) = self.workspace_storage {
            let vp = VirtualPath::parse(&node.virtual_path)
                .map_err(|e| anyhow::anyhow!("invalid virtual path: {e}"))?;
            ws.put_object(&vp, Bytes::from(content.to_owned()), "text/markdown")
                .await
                .map(|_| ())
                .map_err(|e| anyhow::anyhow!("{e}"))
        } else {
            self.workspace_content
                .write(tenant_id, &node.virtual_path, content)
                .await
        };

        if let Err(e) = write_result {
            warn!(
                error = %e,
                path = node.virtual_path,
                "workspace save_document: content write failed"
            );
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
