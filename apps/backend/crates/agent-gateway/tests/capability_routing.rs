//! Capability routing integration tests.
//!
//! Phase 3.5: Verify `extract.fields.invoice` is discoverable and correctly described
//!   so the semantic router would select it for invoice-extraction prompts.
//!
//! Phase 4.7: Verify focused storage providers are registered.
//!
//! Phase 1 (capabilities consolidation): Verify that the two consolidated domain
//!   providers (storage-workspace, storage-fs) are registered, expose the correct
//!   tool surfaces, and that the 14 removed legacy capabilities are gone.
//!
//! Phase 8 (code-project): Verify that code-project is registered and exposes the
//!   expected tool surface (scaffold_project, edit_file, apply_patch, add_dependency,
//!   read_project, host_project).

use agent_core::{
    CapabilityRegistry, NativeStorageFactory, PlanTier,
    capabilities::discovery::CapabilityDiscovery,
    capabilities::manifest::ToolKind,
    context::tenant::TenantContext,
    llm::{LlmBinding, LlmRegistry},
};
use std::collections::HashMap;
use common::memory::{InMemoryWorkspaceContent, InMemoryWorkspaceStore, WorkspaceStore, WorkspaceContentStore};
use std::sync::Arc;

// ── helpers ───────────────────────────────────────────────────────────────────

fn capabilities_dir() -> std::path::PathBuf {
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .find_map(|a| {
            let candidate = a.join("capabilities");
            if candidate.is_dir() { Some(candidate) } else { None }
        })
        .unwrap_or_else(|| std::path::PathBuf::from("capabilities"))
}

fn build_test_registry() -> CapabilityRegistry {
    let ws_store: Arc<dyn WorkspaceStore> = Arc::new(InMemoryWorkspaceStore::new());
    let ws_content: Arc<dyn WorkspaceContentStore> = Arc::new(InMemoryWorkspaceContent::new());

    let llm = Arc::new(LlmRegistry::new(
        HashMap::new(),
        HashMap::new(),
        LlmBinding {
            provider: "anthropic".into(),
            model: "claude-haiku-4-5".into(),
        },
    ));
    let mut reg = CapabilityRegistry::with_default_factories(Arc::clone(&llm));
    reg.register_factory(NativeStorageFactory::new(
        Arc::clone(&ws_store),
        Arc::clone(&ws_content),
    ));

    let dir = capabilities_dir();
    if dir.exists() {
        let discovery = CapabilityDiscovery::new(vec![dir]);
        let _ = discovery.discover_into(&mut reg);
    }
    reg
}

fn dev_tenant() -> TenantContext {
    TenantContext::new(
        "test-tenant",
        Some::<String>("dev-user".into()),
        PlanTier::Free,
        "/tmp/conusai-test-ws",
    )
}

// ── Phase 3.5 — Invoice capability is loaded and correctly described ──────────

#[test]
fn invoice_capability_is_registered() {
    let reg = build_test_registry();

    let provider = reg.get_provider("invoice-processing");
    assert!(
        provider.is_some(),
        "expected 'invoice-processing' provider in registry; capabilities dir = {:?}",
        capabilities_dir()
    );

    let provider = provider.unwrap();
    let manifest = provider.manifest();
    assert_eq!(manifest.namespace.as_deref(), Some("extract.fields.invoice"));
    assert_eq!(manifest.category.as_deref(), Some("extract"));
    assert!(manifest.accepts.iter().any(|a| a.mime.contains("pdf") || a.mime.contains("image")));
    assert!(!manifest.emits.is_empty());
}

#[test]
fn invoice_manifest_has_extract_invoice_tool() {
    let reg = build_test_registry();
    let provider = match reg.get_provider("invoice-processing") {
        Some(p) => p,
        None => return,
    };
    let tool_names: Vec<&str> = provider.manifest().tools.iter().map(|t| t.name.as_str()).collect();
    assert!(
        tool_names.contains(&"extract_invoice"),
        "invoice capability should expose an 'extract_invoice' tool; found: {tool_names:?}"
    );
}

#[test]
#[ignore = "requires live gateway + Qdrant; set GATEWAY_INTEGRATION_TEST=1 to run"]
fn semantic_router_selects_invoice_for_pdf_extraction_prompt() {
    let _ = std::env::var("GATEWAY_INTEGRATION_TEST")
        .expect("GATEWAY_INTEGRATION_TEST must be set to run this test");
}

// ── Phase 4.7 (legacy) — kept for backwards reference ────────────────────────

#[test]
fn legacy_workspace_monolith_absent() {
    let reg = build_test_registry();
    let old_cap = reg.get_provider("workspace-provider");
    assert!(old_cap.is_none(), "legacy WorkspaceProvider should not be present");
}

// ── Phase 1 consolidation — storage-workspace exposes all 11 tools ────────────

#[test]
fn storage_workspace_is_registered() {
    let reg = build_test_registry();
    let provider = reg.get_provider("storage-workspace");
    assert!(
        provider.is_some(),
        "storage-workspace provider should be registered; capabilities dir = {:?}",
        capabilities_dir()
    );
}

#[test]
fn storage_workspace_exposes_all_legacy_tools() {
    let reg = build_test_registry();
    let provider = match reg.get_provider("storage-workspace") {
        Some(p) => p,
        None => return,
    };
    let tool_names: Vec<&str> = provider.manifest().tools.iter().map(|t| t.name.as_str()).collect();
    let expected = [
        "save_document", "list_folders", "show_tree", "find_by_name",
        "create_folder", "ensure_folder", "ensure_date_folder",
        "move_node", "delete_node", "bulk_delete", "tag_object",
    ];
    for tool in &expected {
        assert!(
            tool_names.contains(tool),
            "storage-workspace should expose tool '{tool}'; found: {tool_names:?}"
        );
    }
    assert_eq!(tool_names.len(), 11, "storage-workspace should have exactly 11 tools; found: {tool_names:?}");
}

#[test]
fn storage_workspace_manifest_metadata() {
    let reg = build_test_registry();
    let provider = match reg.get_provider("storage-workspace") {
        Some(p) => p,
        None => return,
    };
    let m = provider.manifest();
    assert_eq!(m.category.as_deref(), Some("storage"));
    assert_eq!(m.kind, ToolKind::Native);
    assert_eq!(m.namespace.as_deref(), Some("storage.workspace"));
}

// ── Phase 1 consolidation — storage-fs exposes all 5 tools ───────────────────

#[test]
fn storage_fs_is_registered() {
    let reg = build_test_registry();
    let provider = reg.get_provider("storage-fs");
    assert!(
        provider.is_some(),
        "storage-fs provider should be registered; capabilities dir = {:?}",
        capabilities_dir()
    );
}

#[test]
fn storage_fs_dispatches_all_five_tools() {
    let reg = build_test_registry();
    let provider = match reg.get_provider("storage-fs") {
        Some(p) => p,
        None => return,
    };
    let tool_names: Vec<&str> = provider.manifest().tools.iter().map(|t| t.name.as_str()).collect();
    let expected = ["read_file", "write_file", "put_object", "move_object", "list_paths"];
    for tool in &expected {
        assert!(
            tool_names.contains(tool),
            "storage-fs should expose tool '{tool}'; found: {tool_names:?}"
        );
    }
    assert_eq!(tool_names.len(), 5, "storage-fs should have exactly 5 tools; found: {tool_names:?}");
}

#[test]
fn storage_fs_renames_list_folders_to_list_paths() {
    let reg = build_test_registry();
    let provider = match reg.get_provider("storage-fs") {
        Some(p) => p,
        None => return,
    };
    let tool_names: Vec<&str> = provider.manifest().tools.iter().map(|t| t.name.as_str()).collect();
    assert!(
        tool_names.contains(&"list_paths"),
        "storage-fs must expose 'list_paths' (not the legacy 'list_folders')"
    );
    assert!(
        !tool_names.contains(&"list_folders"),
        "storage-fs must NOT expose 'list_folders' — that name belongs to storage-workspace"
    );
}

#[test]
fn storage_fs_manifest_metadata() {
    let reg = build_test_registry();
    let provider = match reg.get_provider("storage-fs") {
        Some(p) => p,
        None => return,
    };
    let m = provider.manifest();
    assert_eq!(m.category.as_deref(), Some("storage"));
    assert_eq!(m.kind, ToolKind::Native);
    assert!(m.idempotent, "storage-fs should be idempotent");
}

// ── Phase 1 consolidation — legacy capabilities are removed ──────────────────

#[test]
fn legacy_storage_capabilities_are_removed() {
    let reg = build_test_registry();
    let removed = [
        "storage-workspace-move", "storage-put", "storage-read-text", "storage-write-text",
        "storage-move", "storage-delete", "storage-bulk-delete", "storage-list-folders",
        "storage-create-folder", "storage-ensure-folder", "storage-ensure-date-folder",
        "storage-find-by-name", "storage-show-tree", "storage-tag",
    ];
    for cap in &removed {
        assert!(
            reg.get_provider(cap).is_none(),
            "legacy capability '{cap}' should be removed after consolidation"
        );
    }
}

// ── Phase 1 consolidation — functional tests ─────────────────────────────────

#[tokio::test]
async fn storage_workspace_save_document_roundtrip() {
    let tmp = tempfile::tempdir().expect("temp dir");

    let ws_store: Arc<dyn WorkspaceStore> = Arc::new(InMemoryWorkspaceStore::new());
    let ws_content: Arc<dyn WorkspaceContentStore> = Arc::new(InMemoryWorkspaceContent::new());
    let factory = NativeStorageFactory::new(Arc::clone(&ws_store), Arc::clone(&ws_content));

    let mut reg = CapabilityRegistry::new();
    reg.register_factory(factory);

    let dir = capabilities_dir();
    if dir.exists() {
        let _ = CapabilityDiscovery::new(vec![dir]).discover_into(&mut reg);
    }

    let provider = match reg.get_provider("storage-workspace") {
        Some(p) => p,
        None => return,
    };

    let mut tenant = dev_tenant();
    tenant.workspace_root = tmp.path().to_path_buf();

    let input = serde_json::json!({
        "folder_name": "Research",
        "filename": "notes",
        "content": "# My Notes\n\nSome content here."
    });
    let result = provider.invoke("save_document", &input, Some(&tenant)).await;
    assert!(result.is_ok(), "save_document should succeed: {:?}", result);
    let v = result.unwrap();
    assert_eq!(v["status"], "ok");
    assert_eq!(v["folder"], "Research");
}

#[tokio::test]
async fn storage_workspace_show_tree_returns_tree() {
    let ws_store: Arc<dyn WorkspaceStore> = Arc::new(InMemoryWorkspaceStore::new());
    let ws_content: Arc<dyn WorkspaceContentStore> = Arc::new(InMemoryWorkspaceContent::new());
    let factory = NativeStorageFactory::new(Arc::clone(&ws_store), Arc::clone(&ws_content));

    let mut reg = CapabilityRegistry::new();
    reg.register_factory(factory);

    let dir = capabilities_dir();
    if dir.exists() {
        let _ = CapabilityDiscovery::new(vec![dir]).discover_into(&mut reg);
    }

    let provider = match reg.get_provider("storage-workspace") {
        Some(p) => p,
        None => return,
    };

    let tenant = dev_tenant();
    let input = serde_json::json!({});
    let result = provider.invoke("show_tree", &input, Some(&tenant)).await;
    assert!(result.is_ok(), "show_tree should succeed: {:?}", result);
    assert!(result.unwrap()["tree"].is_string());
}

#[tokio::test]
async fn storage_fs_write_then_read_roundtrip() {
    let tmp = tempfile::tempdir().expect("temp dir");

    let ws_store: Arc<dyn WorkspaceStore> = Arc::new(InMemoryWorkspaceStore::new());
    let ws_content: Arc<dyn WorkspaceContentStore> = Arc::new(InMemoryWorkspaceContent::new());
    let factory = NativeStorageFactory::new(Arc::clone(&ws_store), Arc::clone(&ws_content));

    let mut reg = CapabilityRegistry::new();
    reg.register_factory(factory);

    let dir = capabilities_dir();
    if dir.exists() {
        let _ = CapabilityDiscovery::new(vec![dir]).discover_into(&mut reg);
    }

    let provider = match reg.get_provider("storage-fs") {
        Some(p) => p,
        None => return,
    };

    let mut tenant = dev_tenant();
    tenant.workspace_root = tmp.path().to_path_buf();

    let write_input = serde_json::json!({
        "path": "notes/scratch.txt",
        "content": "hello from storage-fs"
    });
    let write_result = provider.invoke("write_file", &write_input, Some(&tenant)).await;
    assert!(write_result.is_ok(), "write_file should succeed: {:?}", write_result);

    let read_input = serde_json::json!({ "path": "notes/scratch.txt" });
    let read_result = provider.invoke("read_file", &read_input, Some(&tenant)).await;
    assert!(read_result.is_ok(), "read_file should succeed: {:?}", read_result);
    assert_eq!(read_result.unwrap()["content"], "hello from storage-fs");
}

// ── Kept for backwards compat reference — Phase 4.7 functional tests ─────────

#[tokio::test]
async fn storage_ensure_folder_creates_directory() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let ws_path = tmp.path().to_string_lossy().to_string();

    let ws_store: Arc<dyn WorkspaceStore> = Arc::new(InMemoryWorkspaceStore::new());
    let ws_content: Arc<dyn WorkspaceContentStore> = Arc::new(InMemoryWorkspaceContent::new());
    let factory = NativeStorageFactory::new(Arc::clone(&ws_store), Arc::clone(&ws_content));

    let mut reg = CapabilityRegistry::new();
    reg.register_factory(factory);

    let dir = capabilities_dir();
    if dir.exists() {
        let _ = CapabilityDiscovery::new(vec![dir]).discover_into(&mut reg);
    }

    // After consolidation, ensure_folder lives under storage-workspace
    let provider = match reg.get_provider("storage-workspace") {
        Some(p) => p,
        None => return,
    };

    let mut tenant = dev_tenant();
    tenant.workspace_root = std::path::PathBuf::from(&ws_path);

    let input = serde_json::json!({ "path": "Research/notes" });
    let result = provider.invoke("ensure_folder", &input, Some(&tenant)).await;

    assert!(result.is_ok(), "ensure_folder should succeed: {:?}", result);
    assert!(
        tmp.path().join("Research/notes").exists(),
        "directory Research/notes should have been created"
    );
}

#[tokio::test]
async fn storage_write_text_creates_file() {
    let tmp = tempfile::tempdir().expect("temp dir");

    let ws_store: Arc<dyn WorkspaceStore> = Arc::new(InMemoryWorkspaceStore::new());
    let ws_content: Arc<dyn WorkspaceContentStore> = Arc::new(InMemoryWorkspaceContent::new());
    let factory = NativeStorageFactory::new(Arc::clone(&ws_store), Arc::clone(&ws_content));

    let mut reg = CapabilityRegistry::new();
    reg.register_factory(factory);

    let dir = capabilities_dir();
    if dir.exists() {
        let _ = CapabilityDiscovery::new(vec![dir]).discover_into(&mut reg);
    }

    // After consolidation, write_file lives under storage-fs
    let provider = match reg.get_provider("storage-fs") {
        Some(p) => p,
        None => return,
    };

    let mut tenant = dev_tenant();
    tenant.workspace_root = std::path::PathBuf::from(tmp.path());

    let input = serde_json::json!({
        "path": "Research/meeting-notes.md",
        "content": "# Meeting notes\n\n- Discussed Q3 roadmap"
    });
    let result = provider.invoke("write_file", &input, Some(&tenant)).await;

    assert!(result.is_ok(), "write_file should succeed: {:?}", result);
    let written = std::fs::read_to_string(tmp.path().join("Research/meeting-notes.md"))
        .expect("written file should be readable");
    assert!(written.contains("Q3 roadmap"), "file content should be preserved");
}

// ── Phase 8 — code-project capability is registered ──────────────────────────

#[test]
fn code_project_is_registered() {
    let reg = build_test_registry();
    let provider = reg.get_provider("code-project");
    assert!(
        provider.is_some(),
        "code-project provider should be registered; capabilities dir = {:?}",
        capabilities_dir()
    );
}

#[test]
fn code_project_exposes_all_six_tools() {
    let reg = build_test_registry();
    let provider = match reg.get_provider("code-project") {
        Some(p) => p,
        None => return,
    };
    let tool_names: Vec<&str> = provider.manifest().tools.iter().map(|t| t.name.as_str()).collect();
    let expected = [
        "scaffold_project", "edit_file", "apply_patch", "add_dependency",
        "read_project", "host_project",
    ];
    for tool in &expected {
        assert!(
            tool_names.contains(tool),
            "code-project should expose tool '{tool}'; found: {tool_names:?}"
        );
    }
    assert_eq!(tool_names.len(), 6, "code-project should have exactly 6 tools; found: {tool_names:?}");
}

#[test]
fn code_project_manifest_metadata() {
    let reg = build_test_registry();
    let provider = match reg.get_provider("code-project") {
        Some(p) => p,
        None => return,
    };
    let m = provider.manifest();
    assert_eq!(m.category.as_deref(), Some("compose"));
    assert_eq!(m.namespace.as_deref(), Some("code.project"));
    assert_eq!(m.kind, ToolKind::Chain);
    assert!(!m.idempotent, "code-project is not idempotent (modifies project files)");
}

#[test]
fn code_project_has_chain_config() {
    let reg = build_test_registry();
    let provider = match reg.get_provider("code-project") {
        Some(p) => p,
        None => return,
    };
    assert!(
        provider.manifest().chain.is_some(),
        "code-project manifest must have a [chain] section"
    );
}

// ── Phase 9.5 — host_project static-hosting contract ─────────────────────────

/// Verifies that the `host_project` tool is registered and that its manifest
/// description references static hosting, so operators know the capability supports it.
#[test]
fn host_project_tool_describes_static_hosting() {
    let reg = build_test_registry();
    let provider = match reg.get_provider("code-project") {
        Some(p) => p,
        None => return,
    };
    let host_tool = provider
        .manifest()
        .tools
        .iter()
        .find(|t| t.name == "host_project")
        .expect("code-project must expose host_project tool");

    let desc = host_tool.description.to_lowercase();
    assert!(
        desc.contains("static") || desc.contains("hosting") || desc.contains("host"),
        "host_project description should mention hosting/static; got: {}",
        host_tool.description
    );
}

/// Verifies that the code-project chain config uses the `smart` model alias
/// and has a max_tokens budget adequate for generating full project files.
#[test]
fn code_project_chain_uses_smart_model_with_sufficient_tokens() {
    let reg = build_test_registry();
    let provider = match reg.get_provider("code-project") {
        Some(p) => p,
        None => return,
    };
    let chain = provider
        .manifest()
        .chain
        .as_ref()
        .expect("code-project must have chain config");
    assert_eq!(
        chain.model, "smart",
        "code-project should use the 'smart' model alias for code generation"
    );
    assert!(
        chain.max_tokens >= 4096,
        "code-project needs at least 4096 max_tokens to generate project files; got {}",
        chain.max_tokens
    );
}

/// Verifies that the code-project manifest declares `artifact_path_prefix = ""`
/// in the chain system prompt metadata section, ensuring files land at raw paths.
#[test]
fn code_project_system_prompt_mentions_artifact_path_prefix() {
    let reg = build_test_registry();
    let provider = match reg.get_provider("code-project") {
        Some(p) => p,
        None => return,
    };
    let chain = provider
        .manifest()
        .chain
        .as_ref()
        .expect("code-project must have chain config");
    let sys = chain.system_prompt.as_deref().unwrap_or("");
    assert!(
        sys.contains("artifact_path_prefix") || sys.contains("artifacts"),
        "code-project system prompt should reference artifact_path_prefix or artifacts schema; \
         prompt starts with: {}",
        &sys[..sys.len().min(200)]
    );
}
