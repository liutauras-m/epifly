/// ContextBuilder — assembles a workspace-scoped system preamble for the agent.
///
/// Walks ancestor folders, loads each folder's CONTEXT.md or README.md from RustFS,
/// then loads the selected conversation body. Concatenates sections with a divider,
/// truncating via the injected `ContextTruncator` strategy (default: oldest-first).
use crate::context::tenant::TenantContext;
use crate::memory::truncator::{ContextTruncator, OldestFirstTruncator};
use common::memory::store::{WorkspaceContentStore, WorkspaceStore};
use common::memory::workspace::{NodeKind, effective_user_id};
use std::sync::Arc;
use tracing::instrument;
use ulid::Ulid;

pub struct ContextBuilder {
    store: Arc<dyn WorkspaceStore>,
    content: Arc<dyn WorkspaceContentStore>,
    truncator: Arc<dyn ContextTruncator>,
}

impl ContextBuilder {
    pub fn new(store: Arc<dyn WorkspaceStore>, content: Arc<dyn WorkspaceContentStore>) -> Self {
        Self {
            store,
            content,
            truncator: Arc::new(OldestFirstTruncator),
        }
    }

    /// Override the truncation strategy.
    pub fn with_truncator(mut self, truncator: Arc<dyn ContextTruncator>) -> Self {
        self.truncator = truncator;
        self
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
            let (key, legacy) = node_content_keys_ref(&selected);
            match self.content.read(tenant_id, key, legacy).await {
                Ok(body) if !body.is_empty() => {
                    sections.push((selected.virtual_path.clone(), body));
                }
                _ => {}
            }
        }

        if sections.is_empty() {
            return String::new();
        }

        // Delegate truncation to the injected strategy.
        self.truncator.truncate(&mut sections, max_chars);

        // Build final string
        let mut out = String::from("# Workspace context\n");
        for (path, body) in sections {
            out.push_str(&format!("\n## {path}\n\n{body}\n\n---\n"));
        }
        out
    }

    /// Try reading each path in order; return the first non-empty body.
    /// Used for folder CONTEXT.md / README.md — these are virtual paths with no object_key.
    async fn load_first(&self, tenant_id: &str, paths: &[String]) -> String {
        for path in paths {
            let body = self
                .content
                .read(tenant_id, path, None)
                .await
                .unwrap_or_default();
            if !body.is_empty() {
                return body;
            }
        }
        String::new()
    }
}

/// Extract `(primary_key, legacy_key)` from a `WorkspaceNode` for content store calls.
fn node_content_keys_ref(node: &common::memory::workspace::WorkspaceNode) -> (&str, Option<&str>) {
    match &node.object_key {
        Some(ok) => (ok.as_str(), Some(node.virtual_path.as_str())),
        None => (node.virtual_path.as_str(), None),
    }
}
