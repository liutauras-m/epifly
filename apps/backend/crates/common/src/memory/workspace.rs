use crate::error::{ConusAiError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ulid::Ulid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    Folder,
    Conversation, // name always ends in ".md"
    File,
}

/// Semantic kind used for workspace UX branching. Distinct from `NodeKind` (which is a
/// storage/mime hint). The UI must branch on this field, not on `mime_type`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceNodeKind {
    Folder,
    #[default]
    File,
    Thread,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkspaceNode {
    pub id: Ulid,
    pub tenant_id: String,
    /// Never None in storage — dev mode maps user_id=None → "__dev__".
    pub owner_id: String,
    pub parent_id: Option<Ulid>,
    pub kind: NodeKind,
    /// Leaf name: "Kickoff.md" or "Acme". No path separators.
    pub name: String,
    /// Full slash-joined path from root, no leading slash.
    /// Example: "Clients/Acme/Kickoff.md"
    pub virtual_path: String,
    pub last_modified: DateTime<Utc>,
    #[serde(default)]
    pub shared_with: Vec<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
    /// Protected root folders cannot be deleted or moved. Only tenant admin deletion cascades through.
    #[serde(default)]
    pub is_protected_root: bool,
    /// Stable S3 content key (Step 3.4 migration). Format: `"nodes/{node_id}/content"`.
    /// `None` for pre-migration nodes and folder nodes (folders have no content body).
    #[serde(default)]
    pub object_key: Option<String>,
    /// Semantic kind for UX branching. Defaults to `File` on old rows; backfilled to `Folder`
    /// on deserialization when `kind == NodeKind::Folder`. See `WorkspaceNodeKind`.
    #[serde(default)]
    pub semantic_kind: WorkspaceNodeKind,
    /// Who produced this node: `"upload"` | `"generated"` | `"thread_projection"`.
    #[serde(default)]
    pub source_type: Option<String>,
    /// For `thread_projection` nodes, the originating thread_id. For `upload`, the upload_id.
    #[serde(default)]
    pub source_id: Option<String>,
    /// Soft-delete timestamp for `Thread`-kind nodes (delete-as-pause, Step 5.6).
    /// `None` = visible; `Some(t)` = hidden since `t`. List endpoints filter `hidden_at IS NULL`.
    #[serde(default)]
    pub hidden_at: Option<DateTime<Utc>>,
    /// User-defined tags for polyhierarchy-lite filtering (Step 5.5).
    /// Normalised: lowercase, trimmed, deduped; max 32 tags, 64 chars each.
    #[serde(default)]
    pub tags: Vec<String>,
}

impl WorkspaceNode {
    pub fn new_folder(
        tenant_id: impl Into<String>,
        owner_id: impl Into<String>,
        parent_id: Option<Ulid>,
        name: impl Into<String>,
        virtual_path: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Ulid::new(),
            tenant_id: tenant_id.into(),
            owner_id: owner_id.into(),
            parent_id,
            kind: NodeKind::Folder,
            name: name.into(),
            virtual_path: virtual_path.into(),
            last_modified: now,
            shared_with: vec![],
            metadata: serde_json::Value::Null,
            is_protected_root: false,
            object_key: None,
            semantic_kind: WorkspaceNodeKind::Folder,
            source_type: None,
            source_id: None,
            hidden_at: None,
            tags: vec![],
        }
    }

    pub fn new_conversation(
        tenant_id: impl Into<String>,
        owner_id: impl Into<String>,
        parent_id: Option<Ulid>,
        name: impl Into<String>,
        virtual_path: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        let id = Ulid::new();
        Self {
            object_key: Some(format!("nodes/{id}/content")),
            id,
            tenant_id: tenant_id.into(),
            owner_id: owner_id.into(),
            parent_id,
            kind: NodeKind::Conversation,
            name: name.into(),
            virtual_path: virtual_path.into(),
            last_modified: now,
            shared_with: vec![],
            metadata: serde_json::Value::Null,
            is_protected_root: false,
            semantic_kind: WorkspaceNodeKind::File,
            source_type: None,
            source_id: None,
            hidden_at: None,
            tags: vec![],
        }
    }
}

/// Validate a node name. Returns `Err(Validation)` on any violation.
pub fn validate_name(name: &str, kind: NodeKind) -> Result<()> {
    if name.is_empty() {
        return Err(ConusAiError::Validation("name must not be empty".into()));
    }
    if name.len() > 255 {
        return Err(ConusAiError::Validation(
            "name too long (max 255 chars)".into(),
        ));
    }
    if name.contains('/') || name.contains('\\') {
        return Err(ConusAiError::Validation(
            "name must not contain path separators".into(),
        ));
    }
    if name.contains("..") {
        return Err(ConusAiError::Validation(
            "name must not contain '..'".into(),
        ));
    }
    if name.starts_with('.') {
        return Err(ConusAiError::Validation(
            "name must not start with '.'".into(),
        ));
    }
    if kind == NodeKind::Conversation && !name.ends_with(".md") {
        return Err(ConusAiError::Validation(
            "conversation names must end with '.md'".into(),
        ));
    }
    Ok(())
}

/// Normalise a list of tags: lowercase, trim, dedupe; enforce max 32 tags, 64 chars each.
/// Returns `Err(Validation)` if any individual tag violates constraints.
pub fn normalize_tags(tags: Vec<String>) -> Result<Vec<String>> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(tags.len().min(32));
    for raw in tags {
        let t = raw.trim().to_lowercase();
        if t.is_empty() {
            continue;
        }
        if t.len() > 64 {
            return Err(ConusAiError::Validation(format!(
                "tag '{}' exceeds 64 characters",
                &t[..20]
            )));
        }
        if seen.insert(t.clone()) {
            if out.len() >= 32 {
                return Err(ConusAiError::Validation(
                    "too many tags (max 32)".into(),
                ));
            }
            out.push(t);
        }
    }
    Ok(out)
}

/// Build a virtual path by joining an optional parent path with a leaf name.
pub fn join_virtual_path(parent_path: Option<&str>, name: &str) -> String {
    match parent_path {
        None | Some("") => name.to_string(),
        Some(p) => format!("{p}/{name}"),
    }
}

/// Resolve effective user_id — maps None (dev mode) to the "__dev__" sentinel.
pub fn effective_user_id(user_id: Option<&str>) -> &str {
    user_id.unwrap_or("__dev__")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_node_serde_roundtrip() {
        let node = WorkspaceNode {
            id: Ulid::new(),
            tenant_id: "acme".into(),
            owner_id: "user-1".into(),
            parent_id: None,
            kind: NodeKind::Folder,
            name: "Clients".into(),
            virtual_path: "Clients".into(),
            last_modified: Utc::now(),
            shared_with: vec!["user-2".into()],
            metadata: serde_json::json!({"color": "blue"}),
            is_protected_root: false,
            object_key: None,
            semantic_kind: WorkspaceNodeKind::Folder,
            source_type: None,
            source_id: None,
            hidden_at: None,
            tags: vec![],
        };
        let json = serde_json::to_string(&node).unwrap();
        let back: WorkspaceNode = serde_json::from_str(&json).unwrap();
        assert_eq!(node.id, back.id);
        assert_eq!(node.kind, back.kind);
        assert_eq!(node.shared_with, back.shared_with);
    }

    #[test]
    fn validate_name_empty() {
        assert!(validate_name("", NodeKind::Folder).is_err());
    }

    #[test]
    fn validate_name_too_long() {
        let name = "a".repeat(256);
        assert!(validate_name(&name, NodeKind::Folder).is_err());
    }

    #[test]
    fn validate_name_slash() {
        assert!(validate_name("a/b", NodeKind::Folder).is_err());
    }

    #[test]
    fn validate_name_dotdot() {
        assert!(validate_name("../etc", NodeKind::Folder).is_err());
    }

    #[test]
    fn validate_name_leading_dot() {
        assert!(validate_name(".hidden", NodeKind::Folder).is_err());
    }

    #[test]
    fn validate_conversation_requires_md() {
        assert!(validate_name("notes", NodeKind::Conversation).is_err());
        assert!(validate_name("notes.md", NodeKind::Conversation).is_ok());
    }

    #[test]
    fn validate_folder_happy_path() {
        assert!(validate_name("My Project 2026", NodeKind::Folder).is_ok());
    }

    #[test]
    fn join_virtual_path_root() {
        assert_eq!(join_virtual_path(None, "Clients"), "Clients");
        assert_eq!(join_virtual_path(Some(""), "Clients"), "Clients");
    }

    #[test]
    fn join_virtual_path_nested() {
        assert_eq!(
            join_virtual_path(Some("Clients/Acme"), "Kickoff.md"),
            "Clients/Acme/Kickoff.md"
        );
    }

    #[test]
    fn effective_user_id_maps_none() {
        assert_eq!(effective_user_id(None), "__dev__");
        assert_eq!(effective_user_id(Some("u1")), "u1");
    }
}
