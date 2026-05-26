//! Forced-capability routing tests (PR 2.A).
//!
//! 2.A.9  — basic inclusion: `tools_for_capability_exact_for_tenant` returns
//!           tool definitions for a known, enabled capability.
//! 2.A.10 — security / tenant validation: unknown or scoped-out capabilities
//!           return `None` so the gateway never pins untrusted tools.
//!
//! The `merge_pinned` helper (position-guarantee + truncation) is tested as
//! inline unit tests inside `src/routes/agent.rs` because the function lives
//! in the binary crate and is not re-exported through a library target.

use agent_core::{
    CapabilityRegistry, NativeStorageFactory,
    capabilities::discovery::CapabilityDiscovery,
    llm::{LlmBinding, LlmRegistry},
};
use common::memory::{
    InMemoryWorkspaceContent, InMemoryWorkspaceStore, WorkspaceContentStore, WorkspaceStore,
};
use std::collections::HashMap;
use std::sync::Arc;

// ── helpers ───────────────────────────────────────────────────────────────────

fn capabilities_dir() -> std::path::PathBuf {
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .find_map(|a| {
            let candidate = a.join("capabilities");
            if candidate.is_dir() {
                Some(candidate)
            } else {
                None
            }
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
        let _ = CapabilityDiscovery::new(vec![dir]).discover_into(&mut reg);
    }
    reg
}

fn dev_tenant_id() -> &'static str {
    "test-tenant"
}

// ── 2.A.9 — basic inclusion ───────────────────────────────────────────────────

/// `tools_for_capability_exact_for_tenant` must return at least one tool
/// definition for every capability that is enabled and visible to the caller.
#[test]
fn known_capability_returns_tool_definitions() {
    let reg = build_test_registry();
    // storage-workspace is always registered; use it as a stable anchor.
    let result = reg.tools_for_capability_exact_for_tenant("storage-workspace", dev_tenant_id());
    assert!(
        result.is_some(),
        "expected Some(_) for storage-workspace; capabilities dir = {:?}",
        capabilities_dir()
    );
    let tools = result.unwrap();
    assert!(
        !tools.is_empty(),
        "storage-workspace must expose at least one tool"
    );
    // Each entry must be a JSON object with a "name" key.
    for (i, t) in tools.iter().enumerate() {
        assert!(
            t.get("name").and_then(|v| v.as_str()).is_some(),
            "tool[{i}] must have a string 'name' field; got {t}"
        );
    }
}

/// Verify that all four chain-type capabilities that carry `forced_capability`
/// in typical UI flows (storage-workspace, code-project, invoice-processing,
/// ocr-service) return tool defs.
#[test]
fn all_ui_invokable_capabilities_return_tool_definitions() {
    let reg = build_test_registry();
    let caps = [
        "storage-workspace",
        "code-project",
        "invoice-processing",
        "ocr-service",
    ];
    for cap in &caps {
        let result = reg.tools_for_capability_exact_for_tenant(cap, dev_tenant_id());
        // If the capability dir is present the capability must exist; if not present
        // we skip (test environment may not have the capabilities dir).
        if capabilities_dir().exists() {
            assert!(
                result.is_some(),
                "expected tool defs for '{cap}'; got None — is it registered?"
            );
            assert!(
                !result.unwrap().is_empty(),
                "'{cap}' must expose at least one tool"
            );
        }
    }
}

// ── 2.A.10 — security / tenant validation ────────────────────────────────────

/// An unknown capability name must return `None` — the gateway must not panic
/// or return an empty list that could be mistaken for "no tools needed".
#[test]
fn unknown_capability_returns_none() {
    let reg = build_test_registry();
    let result =
        reg.tools_for_capability_exact_for_tenant("non-existent-capability-xyzzy", dev_tenant_id());
    assert!(
        result.is_none(),
        "unknown capability must return None, not an empty Vec"
    );
}

/// A capability that is disabled must return `None` even when the name is
/// otherwise valid.  This prevents a UI caller from forcing tools that an
/// operator has administratively disabled.
#[test]
fn disabled_capability_returns_none() {
    let mut reg = build_test_registry();
    // Register storage-workspace, then disable it.
    if reg.get_provider("storage-workspace").is_none() {
        return; // capabilities dir not present; skip
    }
    reg.set_enabled("storage-workspace", false);
    let result = reg.tools_for_capability_exact_for_tenant("storage-workspace", dev_tenant_id());
    assert!(
        result.is_none(),
        "disabled capability must return None from tools_for_capability_exact_for_tenant"
    );
}

/// A capability scoped to a specific tenant must not be returned for a
/// different tenant's `forced_capability` request.
///
/// `tools_for_capability_exact_for_tenant` short-circuits at the visibility
/// check before reaching the provider layer, so it returns `None` for an
/// out-of-scope tenant even when the capability name is valid.
/// We verify the negative case (`None` for wrong tenant) and the positive
/// side via `enabled_for_tenant` (which is the internal gate that drives the
/// visibility check used by `tools_for_capability_exact_for_tenant`).
#[test]
fn scoped_capability_hidden_from_wrong_tenant() {
    use agent_core::capabilities::card::CapabilityCard;
    use agent_core::capabilities::manifest::{ToolKind, ToolManifest};

    let mut reg = CapabilityRegistry::new();

    // Manually register a capability scoped only to "acme".
    let manifest = ToolManifest {
        name: "acme-only-cap".into(),
        version: "0.1.0".into(),
        description: "acme exclusive".into(),
        kind: ToolKind::Chain,
        tools: vec![],
        config: serde_json::Value::Null,
        tags: vec![],
        namespace: None,
        chain: None,
        tenant_scope: vec!["acme".into()],
        enabled: true,
        search_keywords: vec![],
        schema_version: "2.0".into(),
        category: None,
        accepts: vec![],
        emits: vec![],
        idempotent: false,
        cost_hint: None,
        requires: vec![],
    };
    reg.register(CapabilityCard::new(
        manifest,
        std::path::PathBuf::from("/tmp"),
    ));

    // Positive side: "acme" can see the capability in the enabled set.
    let acme_caps: Vec<_> = reg.enabled_for_tenant("acme").collect();
    assert!(
        acme_caps.iter().any(|c| c.manifest.name == "acme-only-cap"),
        "acme should see acme-only-cap in enabled_for_tenant"
    );
    let other_caps: Vec<_> = reg.enabled_for_tenant("other").collect();
    assert!(
        !other_caps
            .iter()
            .any(|c| c.manifest.name == "acme-only-cap"),
        "tenant 'other' must NOT see acme-only-cap in enabled_for_tenant"
    );

    // Security gate: `tools_for_capability_exact_for_tenant` must return None
    // for the wrong tenant — the visibility check fires before the provider
    // lookup, so the gateway never pins the tools.
    let for_other = reg.tools_for_capability_exact_for_tenant("acme-only-cap", "other");
    assert!(
        for_other.is_none(),
        "tenant 'other' must not receive tool defs for a capability scoped to 'acme'"
    );
}
