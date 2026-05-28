/// ContextBuilder — assembles a workspace-scoped system preamble for the agent.
///
/// Walks ancestor folders, loads each folder's CONTEXT.md or README.md from RustFS,
/// then loads the selected conversation body. Concatenates sections with a divider,
/// truncating via the injected `ContextTruncator` strategy (default: oldest-first).
///
/// ## Sibling bias (Step 6.3)
///
/// When `sibling_bias = true`, sibling documents (nodes in the same folder,
/// excluding folders and the thread itself) are loaded and prepended before the
/// conversation body. This biases the agent toward folder-local knowledge.
/// Controlled via `CONUS_WORKSPACE_SIBLING_BIAS=1`; default off.
use crate::context::tenant::TenantContext;
use crate::memory::truncator::{ContextTruncator, OldestFirstTruncator};
use common::memory::store::{WorkspaceContentStore, WorkspaceStore};
use common::memory::workspace::{NodeKind, effective_user_id};
use std::sync::Arc;
use tracing::instrument;
use ulid::Ulid;

/// Maximum siblings injected when sibling_bias is enabled.
/// Prevents sibling noise from swamping the context window.
const MAX_SIBLING_SECTIONS: usize = 3;

pub struct ContextBuilder {
    store: Arc<dyn WorkspaceStore>,
    content: Arc<dyn WorkspaceContentStore>,
    truncator: Arc<dyn ContextTruncator>,
    /// Step 6.3 — when true, include up to `MAX_SIBLING_SECTIONS` sibling documents
    /// in the context so the agent is biased toward folder-local knowledge.
    sibling_bias: bool,
}

impl ContextBuilder {
    pub fn new(store: Arc<dyn WorkspaceStore>, content: Arc<dyn WorkspaceContentStore>) -> Self {
        Self {
            store,
            content,
            truncator: Arc::new(OldestFirstTruncator),
            sibling_bias: false,
        }
    }

    /// Override the truncation strategy.
    pub fn with_truncator(mut self, truncator: Arc<dyn ContextTruncator>) -> Self {
        self.truncator = truncator;
        self
    }

    /// Step 6.3 — enable or disable sibling bias.
    ///
    /// When `true`, up to `MAX_SIBLING_SECTIONS` sibling documents are included
    /// before the conversation body so the agent sees folder-local material first.
    /// Default: `false`.
    pub fn with_sibling_bias(mut self, enabled: bool) -> Self {
        self.sibling_bias = enabled;
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

        // Step 6.3 — sibling bias: inject up to MAX_SIBLING_SECTIONS sibling documents
        // before the conversation body so they rank above unrelated content.
        if self.sibling_bias && selected.kind == NodeKind::Conversation {
            let siblings = self
                .store
                .list_accessible_children(tenant_id, user_id, selected.parent_id)
                .await
                .unwrap_or_default();

            let mut sibling_count = 0;
            for sibling in &siblings {
                if sibling_count >= MAX_SIBLING_SECTIONS {
                    break;
                }
                // Exclude the thread itself, folders, and empty nodes.
                if sibling.id == selected.id || sibling.kind == NodeKind::Folder {
                    continue;
                }
                let (key, legacy) = node_content_keys_ref(sibling);
                if let Ok(body) = self.content.read(tenant_id, key, legacy).await
                    && !body.is_empty()
                {
                    sections.push((sibling.virtual_path.clone(), body));
                    sibling_count += 1;
                }
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::tenant::{PlanTier, TenantContext};
    use common::memory::workspace::{WorkspaceNode, WorkspaceNodeKind};
    use common::memory::{InMemoryWorkspaceContent, InMemoryWorkspaceStore};
    use std::sync::Arc;
    use ulid::Ulid;

    fn test_tenant() -> TenantContext {
        TenantContext::new("acme", Some("__system__"), PlanTier::Enterprise, "/tmp")
    }

    /// Helper: insert a Conversation node (virtual_path as key) with content in the store.
    async fn insert_conversation(
        ws: &Arc<dyn common::memory::store::WorkspaceStore>,
        wc: &Arc<dyn common::memory::store::WorkspaceContentStore>,
        parent_id: Option<Ulid>,
        name: &str,
        virtual_path: &str,
        content: &str,
    ) -> WorkspaceNode {
        let mut node =
            WorkspaceNode::new_conversation("acme", "__system__", parent_id, name, virtual_path);
        node.semantic_kind = WorkspaceNodeKind::Thread;
        ws.upsert_node(node.clone()).await.unwrap();
        // new_conversation always sets object_key; write with that key so
        // node_content_keys_ref's primary-key lookup finds the content.
        let key = node
            .object_key
            .as_deref()
            .unwrap_or(virtual_path);
        wc.write("acme", key, None, content).await.unwrap();
        node
    }

    // ── sibling_bias = false: siblings not included ───────────────────────────

    #[tokio::test]
    async fn without_sibling_bias_siblings_excluded() {
        let ws: Arc<dyn common::memory::store::WorkspaceStore> =
            Arc::new(InMemoryWorkspaceStore::new());
        let wc: Arc<dyn common::memory::store::WorkspaceContentStore> =
            Arc::new(InMemoryWorkspaceContent::new());

        // Create a folder
        let folder = ws
            .create_folder("acme", "__system__", None, "Work")
            .await
            .unwrap();

        // Thread node and a sibling document
        let thread = insert_conversation(
            &ws,
            &wc,
            Some(folder.id),
            "chat.md",
            "Work/chat.md",
            "## Thread body",
        )
        .await;
        let _sibling = insert_conversation(
            &ws,
            &wc,
            Some(folder.id),
            "spec.md",
            "Work/spec.md",
            "## Spec content",
        )
        .await;

        let builder = ContextBuilder::new(Arc::clone(&ws), Arc::clone(&wc));
        // sibling_bias defaults to false
        let ctx = builder
            .build_for_node(&test_tenant(), thread.id, 8000)
            .await;

        assert!(ctx.contains("Thread body"), "thread body must appear");
        assert!(
            !ctx.contains("Spec content"),
            "sibling must NOT appear when sibling_bias is off"
        );
    }

    // ── sibling_bias = true: sibling docs included before thread body ─────────

    #[tokio::test]
    async fn with_sibling_bias_sibling_content_appears_before_thread_body() {
        let ws: Arc<dyn common::memory::store::WorkspaceStore> =
            Arc::new(InMemoryWorkspaceStore::new());
        let wc: Arc<dyn common::memory::store::WorkspaceContentStore> =
            Arc::new(InMemoryWorkspaceContent::new());

        let folder = ws
            .create_folder("acme", "__system__", None, "Work")
            .await
            .unwrap();

        let thread = insert_conversation(
            &ws,
            &wc,
            Some(folder.id),
            "chat.md",
            "Work/chat.md",
            "## Thread body",
        )
        .await;
        let _sibling = insert_conversation(
            &ws,
            &wc,
            Some(folder.id),
            "spec.md",
            "Work/spec.md",
            "## Spec content",
        )
        .await;

        let builder = ContextBuilder::new(Arc::clone(&ws), Arc::clone(&wc)).with_sibling_bias(true);
        let ctx = builder
            .build_for_node(&test_tenant(), thread.id, 8000)
            .await;

        assert!(ctx.contains("Thread body"), "thread body must appear");
        assert!(
            ctx.contains("Spec content"),
            "sibling content must appear when sibling_bias is on"
        );

        // Sibling must rank BEFORE the thread body (earlier in the string).
        let sibling_pos = ctx.find("Spec content").unwrap();
        let thread_pos = ctx.find("Thread body").unwrap();
        assert!(
            sibling_pos < thread_pos,
            "sibling (pos {sibling_pos}) must appear before thread body (pos {thread_pos})"
        );
    }

    // ── sibling_bias caps at MAX_SIBLING_SECTIONS ─────────────────────────────

    #[tokio::test]
    async fn sibling_bias_caps_at_max_sections() {
        let ws: Arc<dyn common::memory::store::WorkspaceStore> =
            Arc::new(InMemoryWorkspaceStore::new());
        let wc: Arc<dyn common::memory::store::WorkspaceContentStore> =
            Arc::new(InMemoryWorkspaceContent::new());

        let folder = ws
            .create_folder("acme", "__system__", None, "Work")
            .await
            .unwrap();

        let thread = insert_conversation(
            &ws,
            &wc,
            Some(folder.id),
            "chat.md",
            "Work/chat.md",
            "## Thread",
        )
        .await;
        for i in 0..MAX_SIBLING_SECTIONS + 2 {
            insert_conversation(
                &ws,
                &wc,
                Some(folder.id),
                &format!("doc{i}.md"),
                &format!("Work/doc{i}.md"),
                &format!("## Sibling {i}"),
            )
            .await;
        }

        let builder = ContextBuilder::new(Arc::clone(&ws), Arc::clone(&wc)).with_sibling_bias(true);
        let ctx = builder
            .build_for_node(&test_tenant(), thread.id, 80_000)
            .await;

        // Count "## Sibling " occurrences — must be ≤ MAX_SIBLING_SECTIONS
        let sibling_count = ctx.matches("## Sibling ").count();
        assert!(
            sibling_count <= MAX_SIBLING_SECTIONS,
            "expected ≤ {MAX_SIBLING_SECTIONS} siblings, got {sibling_count}"
        );
    }

    // ── sibling_bias excludes folders ─────────────────────────────────────────

    #[tokio::test]
    async fn sibling_bias_excludes_folder_nodes() {
        let ws: Arc<dyn common::memory::store::WorkspaceStore> =
            Arc::new(InMemoryWorkspaceStore::new());
        let wc: Arc<dyn common::memory::store::WorkspaceContentStore> =
            Arc::new(InMemoryWorkspaceContent::new());

        let folder = ws
            .create_folder("acme", "__system__", None, "Work")
            .await
            .unwrap();
        let _subfolder = ws
            .create_folder("acme", "__system__", Some(folder.id), "Sub")
            .await
            .unwrap();
        let thread = insert_conversation(
            &ws,
            &wc,
            Some(folder.id),
            "chat.md",
            "Work/chat.md",
            "## Thread body",
        )
        .await;

        let builder = ContextBuilder::new(Arc::clone(&ws), Arc::clone(&wc)).with_sibling_bias(true);
        let ctx = builder
            .build_for_node(&test_tenant(), thread.id, 8000)
            .await;

        // Context should contain the thread body but no folder content (folders have no body).
        assert!(ctx.contains("Thread body"));
        // The subfolder "Sub" itself has no content — the context shouldn't have a section for it.
        assert!(
            !ctx.contains("\n## Work/Sub\n"),
            "folders must not appear in sibling bias sections"
        );
    }
}
