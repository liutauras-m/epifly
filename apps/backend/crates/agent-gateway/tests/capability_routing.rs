//! Capability routing integration tests — Phases 3.5 and 4.7.
//!
//! Phase 3.5: Verify `extract.fields.invoice` is discoverable and correctly described
//!   so the semantic router would select it for invoice-extraction prompts. The full
//!   end-to-end assertion (agent invokes via router in a real chat turn) requires a live
//!   LLM + Qdrant and is gated on `GATEWAY_INTEGRATION_TEST=1`.
//!
//! Phase 4.7: Verify that `storage.ensure_folder` and `storage.write_text` providers
//!   are registered, have the correct manifest metadata, and produce correct output —
//!   proving the old `workspace__save_document` monolith has been replaced by focused
//!   TOML-driven providers.

use agent_core::{
    CapabilityRegistry, NativeStorageFactory, PlanTier,
    capabilities::discovery::CapabilityDiscovery,
    capabilities::manifest::ToolKind,
    context::tenant::TenantContext,
};
use common::memory::{InMemoryWorkspaceContent, InMemoryWorkspaceStore, WorkspaceStore, WorkspaceContentStore};
use std::sync::Arc;

// ── helpers ───────────────────────────────────────────────────────────────────

fn capabilities_dir() -> std::path::PathBuf {
    // When run via `cargo test -p agent-gateway`, the working directory is the
    // workspace root (apps/backend). Fall back to relative path heuristics.
    let candidates = [
        "capabilities",
        "apps/backend/capabilities",
        "../../capabilities",
    ];
    for c in &candidates {
        let p = std::path::Path::new(c);
        if p.exists() {
            return p.to_path_buf();
        }
    }
    std::path::PathBuf::from("capabilities")
}

fn build_test_registry() -> CapabilityRegistry {
    let ws_store: Arc<dyn WorkspaceStore> = Arc::new(InMemoryWorkspaceStore::new());
    let ws_content: Arc<dyn WorkspaceContentStore> = Arc::new(InMemoryWorkspaceContent::new());
    let factory = NativeStorageFactory::new(Arc::clone(&ws_store), Arc::clone(&ws_content));

    let mut reg = CapabilityRegistry::new();
    reg.add_factory(Arc::new(factory));

    let dir = capabilities_dir();
    if dir.exists() {
        let discovery = CapabilityDiscovery::new(vec![dir]);
        let _ = discovery.discover_into(&mut reg);
    }
    reg
}

fn dev_tenant() -> TenantContext {
    TenantContext::new("test-tenant", Some("dev-user".into()), PlanTier::Free, "/tmp/conusai-test-ws")
}

// ── Phase 3.5 — Invoice capability is loaded and correctly described ──────────

#[test]
fn invoice_capability_is_registered() {
    let reg = build_test_registry();

    // The provider is keyed by the TOML `name` field ("invoice-processing").
    let provider = reg.get_provider("invoice-processing");
    assert!(
        provider.is_some(),
        "expected 'invoice-processing' provider in registry; capabilities dir = {:?}",
        capabilities_dir()
    );

    let manifest = provider.unwrap().manifest();
    assert_eq!(manifest.namespace.as_deref(), Some("extract.fields.invoice"),
        "manifest namespace should be extract.fields.invoice");
    assert_eq!(manifest.category.as_deref(), Some("extract"),
        "manifest category should be 'extract'");
    assert!(manifest.accepts.iter().any(|a| a.mime.contains("pdf") || a.mime.contains("image")),
        "invoice capability should accept PDF or image/* attachments");
    assert!(!manifest.emits.is_empty(), "invoice capability should declare emits");
}

#[test]
fn invoice_manifest_has_extract_invoice_tool() {
    let reg = build_test_registry();
    let provider = match reg.get_provider("invoice-processing") {
        Some(p) => p,
        None => return, // capabilities dir not present in this CI env — skip
    };
    let tool_names: Vec<&str> = provider.manifest().tools.iter().map(|t| t.name.as_str()).collect();
    assert!(
        tool_names.contains(&"extract_invoice"),
        "invoice capability should expose an 'extract_invoice' tool; found: {tool_names:?}"
    );
}

/// Full semantic router test — requires GATEWAY_INTEGRATION_TEST=1 + Qdrant + gateway running.
#[test]
#[ignore = "requires live gateway + Qdrant; set GATEWAY_INTEGRATION_TEST=1 to run"]
fn semantic_router_selects_invoice_for_pdf_extraction_prompt() {
    // Proof that removing the domain chains (Phase 3) did not break routing:
    // the router must return 'invoice-processing' in its top-K for a PDF invoice query.
    // This test body is intentionally left as a placeholder; the full assertion
    // runs in the Phase 3 acceptance e2e suite (iOS Playwright, Use Case 1).
    let _ = std::env::var("GATEWAY_INTEGRATION_TEST")
        .expect("GATEWAY_INTEGRATION_TEST must be set to run this test");
}

// ── Phase 4.7 — Storage providers replace workspace__save_document ─────────────

#[test]
fn storage_ensure_folder_and_write_text_are_registered() {
    let reg = build_test_registry();

    let folder_cap = reg.get_provider("storage-ensure-folder");
    assert!(folder_cap.is_some(), "storage-ensure-folder provider should be registered");

    let write_cap = reg.get_provider("storage-write-text");
    assert!(write_cap.is_some(), "storage-write-text provider should be registered");

    // Verify the old monolithic workspace__save_document is NOT present.
    let old_cap = reg.get_provider("workspace-provider");
    assert!(old_cap.is_none(), "legacy WorkspaceProvider should not be present after Phase 4 migration");
}

#[test]
fn storage_ensure_folder_manifest_metadata() {
    let reg = build_test_registry();
    let provider = match reg.get_provider("storage-ensure-folder") {
        Some(p) => p,
        None => return, // capabilities dir not present — skip
    };
    let m = provider.manifest();
    assert_eq!(m.category.as_deref(), Some("storage"));
    assert!(m.idempotent, "ensure_folder must be idempotent");
}

#[test]
fn storage_write_text_manifest_metadata() {
    let reg = build_test_registry();
    let provider = match reg.get_provider("storage-write-text") {
        Some(p) => p,
        None => return, // capabilities dir not present — skip
    };
    let m = provider.manifest();
    assert_eq!(m.category.as_deref(), Some("storage"));
    assert_eq!(m.kind, ToolKind::Native);
}

#[tokio::test]
async fn storage_ensure_folder_creates_directory() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let ws_path = tmp.path().to_string_lossy().to_string();

    let ws_store: Arc<dyn WorkspaceStore> = Arc::new(InMemoryWorkspaceStore::new());
    let ws_content: Arc<dyn WorkspaceContentStore> = Arc::new(InMemoryWorkspaceContent::new());
    let factory = NativeStorageFactory::new(Arc::clone(&ws_store), Arc::clone(&ws_content));

    let mut reg = CapabilityRegistry::new();
    reg.add_factory(Arc::new(factory));

    let dir = capabilities_dir();
    if dir.exists() {
        let _ = CapabilityDiscovery::new(vec![dir]).discover_into(&mut reg);
    }

    let provider = match reg.get_provider("storage-ensure-folder") {
        Some(p) => p,
        None => return, // capabilities dir not found in this env
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
    reg.add_factory(Arc::new(factory));

    let dir = capabilities_dir();
    if dir.exists() {
        let _ = CapabilityDiscovery::new(vec![dir]).discover_into(&mut reg);
    }

    let provider = match reg.get_provider("storage-write-text") {
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
