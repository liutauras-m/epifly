/// ContextBuilder — assembles a workspace-scoped system preamble for the agent.
///
/// Walks ancestor folders, loads each folder's CONTEXT.md or README.md from MinIO,
/// then loads the selected conversation body. Concatenates sections with a divider,
/// truncating from the oldest ancestor first when the total exceeds max_chars.
use crate::context::tenant::TenantContext;
use common::memory::store::{WorkspaceContentStore, WorkspaceStore};
use common::memory::workspace::{NodeKind, effective_user_id};
use std::sync::Arc;
use tracing::instrument;
use ulid::Ulid;

pub struct ContextBuilder {
    store: Arc<dyn WorkspaceStore>,
    content: Arc<dyn WorkspaceContentStore>,
}

impl ContextBuilder {
    pub fn new(store: Arc<dyn WorkspaceStore>, content: Arc<dyn WorkspaceContentStore>) -> Self {
        Self { store, content }
    }

    /// Build a system message string for the agent, scoped to `node_id`.
    ///
    /// Returns an empty string if the node is inaccessible or has no ancestors.
    /// Never fails hard — errors are silently skipped so the agent still works without context.
    #[instrument(skip(self), fields(tenant_id = %tenant.tenant_id, %node_id))]
    pub async fn build_for_node(
        &self,
        tenant: &TenantContext,
        node_id: Ulid,
        max_chars: usize,
    ) -> String {
        let user_id = effective_user_id(tenant.user_id.as_deref());
        let tenant_id = &tenant.tenant_id;

        // Get ancestors root → immediate parent
        let ancestors = match self.store.get_ancestors(tenant_id, user_id, node_id).await {
            Ok(a) => a,
            Err(_) => return String::new(),
        };

        // Get the selected node itself
        let selected = match self
            .store
            .get_accessible_node(tenant_id, user_id, node_id)
            .await
        {
            Ok(n) => n,
            Err(_) => return String::new(),
        };

        let mut sections: Vec<(String, String)> = vec![];

        // Load ancestor folder context files
        for ancestor in &ancestors {
            if ancestor.kind != NodeKind::Folder {
                continue;
            }
            let folder_path = &ancestor.virtual_path;
            let body = self
                .load_first(
                    tenant_id,
                    &[
                        format!("{folder_path}/CONTEXT.md"),
                        format!("{folder_path}/README.md"),
                    ],
                )
                .await;
            if !body.is_empty() {
                sections.push((ancestor.virtual_path.clone(), body));
            }
        }

        // Load selected conversation body
        if selected.kind == NodeKind::Conversation {
            match self.content.read(tenant_id, &selected.virtual_path).await {
                Ok(body) if !body.is_empty() => {
                    sections.push((selected.virtual_path.clone(), body));
                }
                _ => {}
            }
        }

        if sections.is_empty() {
            return String::new();
        }

        // Truncate from the front (oldest ancestor) if total exceeds max_chars
        let mut total: usize = sections.iter().map(|(_, b)| b.len()).sum();
        while total > max_chars && sections.len() > 1 {
            let removed = sections.remove(0);
            total -= removed.1.len();
        }

        // Build final string
        let mut out = String::from("# Workspace context\n");
        for (path, body) in sections {
            out.push_str(&format!("\n## {path}\n\n{body}\n\n---\n"));
        }
        out
    }

    /// Try reading each path in order; return the first non-empty body.
    async fn load_first(&self, tenant_id: &str, paths: &[String]) -> String {
        for path in paths {
            let body = self.content.read(tenant_id, path).await.unwrap_or_default();
            if !body.is_empty() {
                return body;
            }
        }
        String::new()
    }
}
