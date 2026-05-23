//! Lexical capability prefilter integration tests (PR 2.B).
//!
//! 2.B.6 — Verify that `lexical_hint_capabilities` correctly identifies
//!          capabilities for 10 canonical natural-language prompts against the
//!          real capability manifests on disk, and returns empty for a prompt
//!          that should not match any capability keyword.
//!
//! These tests use the full capability registry loaded from the `capabilities/`
//! directory so they exercise both capability-level and per-tool `search_keywords`
//! as they appear in production TOML manifests.

use agent_core::{
    CapabilityRegistry, NativeStorageFactory,
    capabilities::discovery::CapabilityDiscovery,
    llm::{LlmBinding, LlmRegistry},
};
use common::memory::{InMemoryWorkspaceContent, InMemoryWorkspaceStore, WorkspaceStore, WorkspaceContentStore};
use std::collections::HashMap;
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
        LlmBinding { provider: "anthropic".into(), model: "claude-haiku-4-5".into() },
    ));
    let mut reg = CapabilityRegistry::with_default_factories(Arc::clone(&llm));
    reg.register_factory(NativeStorageFactory::new(
        Arc::clone(&ws_store),
        Arc::clone(&ws_content),
    ));
    let dir = capabilities_dir();
    if dir.exists() {
        let _ = CapabilityDiscovery::new(vec![dir]).discover_into(&mut reg);
    }
    reg
}

const TENANT: &str = "test-tenant";

/// Assert that `lexical_hint_capabilities` returns at least one of the
/// `expected_caps` for `prompt`. Skips when capabilities dir is absent.
fn assert_lexical_match(reg: &CapabilityRegistry, prompt: &str, expected_caps: &[&str]) {
    if !capabilities_dir().exists() {
        return; // CI without capabilities dir — skip live manifest tests
    }
    let hits = reg.lexical_hint_capabilities(prompt, TENANT);
    let matched_any = expected_caps.iter().any(|cap| hits.contains(&cap.to_string()));
    assert!(
        matched_any,
        "prompt {prompt:?} should have matched one of {expected_caps:?}; \
         lexical_hint_capabilities returned: {hits:?}"
    );
}

// ── Canonical prompts 1–10 ────────────────────────────────────────────────────

/// 1. "save notes to a folder" — storage-workspace cap-level keyword "save"
#[test]
fn lexical_save_notes_to_folder() {
    let reg = build_test_registry();
    assert_lexical_match(&reg, "save my notes to a folder", &["storage-workspace"]);
}

/// 2. "delete this document" — storage-workspace tool-level keyword "delete"
#[test]
fn lexical_delete_document() {
    let reg = build_test_registry();
    assert_lexical_match(&reg, "delete this document from my workspace", &["storage-workspace"]);
}

/// 3. "scaffold a new React project" — code-project cap-level keyword "scaffold"
#[test]
fn lexical_scaffold_react_project() {
    let reg = build_test_registry();
    assert_lexical_match(&reg, "scaffold a new React project for me", &["code-project"]);
}

/// 4. "add dependency lodash" — code-project tool-level keyword "add dependency"
#[test]
fn lexical_add_dependency() {
    let reg = build_test_registry();
    assert_lexical_match(&reg, "add dependency lodash to the project", &["code-project"]);
}

/// 5. "upload this file" — file-storage tool-level keyword "upload"
#[test]
fn lexical_upload_file() {
    let reg = build_test_registry();
    assert_lexical_match(&reg, "upload this file to cloud storage", &["file-storage"]);
}

/// 6. "extract text from scanned document" — ocr-service cap/tool-level keyword "extract text"
#[test]
fn lexical_extract_text_from_scan() {
    let reg = build_test_registry();
    assert_lexical_match(
        &reg,
        "extract text from this scanned document",
        &["ocr-service", "extract-ocr-vision"],
    );
}

/// 7. "process invoice PDF" — invoice-processing cap-level keyword "invoice"
#[test]
fn lexical_invoice_processing() {
    let reg = build_test_registry();
    assert_lexical_match(&reg, "process this invoice PDF and extract the amounts", &["invoice-processing"]);
}

/// 8. "deploy and get a live URL" — code-project tool-level keyword "host"
#[test]
fn lexical_deploy_app() {
    let reg = build_test_registry();
    assert_lexical_match(&reg, "deploy my app and get a live URL", &["code-project"]);
}

/// 9. "create a new folder" — storage-workspace cap-level keyword "create folder"
#[test]
fn lexical_create_folder() {
    let reg = build_test_registry();
    assert_lexical_match(&reg, "create a new folder for my research", &["storage-workspace"]);
}

/// 10. "remove all files from the archive" — storage-workspace tool-level keyword "remove all"
#[test]
fn lexical_remove_all_files() {
    let reg = build_test_registry();
    assert_lexical_match(&reg, "remove all files from the archive folder", &["storage-workspace"]);
}

// ── Negative test ─────────────────────────────────────────────────────────────

/// A general math / knowledge question should not match any capability's
/// search keywords — lexical filter must return empty, not a spurious hit.
#[test]
fn lexical_no_match_for_math_question() {
    let reg = build_test_registry();
    if !capabilities_dir().exists() {
        return;
    }
    let hits = reg.lexical_hint_capabilities("what is the square root of 144", TENANT);
    assert!(
        hits.is_empty(),
        "pure math question should not match any capability keywords; got: {hits:?}"
    );
}
