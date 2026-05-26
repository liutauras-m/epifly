//! Read-before-write field tests (PR 2.D).
//!
//! Covers two layers:
//!
//! **2.D.5a — TOML round-trip**
//!   `ToolDef.read_before_write` must survive serde serialisation/deserialisation
//!   in TOML manifests.  The `code-project` capability's `add_dependency` tool
//!   is the canonical real-world consumer; if the capabilities directory is
//!   present we load and assert on the live manifest.
//!
//! **2.D.5b — JSON serde**
//!   `read_before_write` must also round-trip through JSON (used in API transport
//!   and in programmatic `CapabilitySpec` payloads).
//!
//! The gateway-level injection function (`maybe_inject_current_content`) lives
//! inside the binary crate and is tested separately via inline `#[cfg(test)]`
//! unit tests in `src/routes/agent.rs`.

use agent_core::capabilities::manifest::{ToolDef, ToolManifest};

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

// ── 2.D.5a — TOML round-trip ─────────────────────────────────────────────────

/// A hand-crafted TOML manifest with `read_before_write` set on a tool must
/// deserialise to `Some("manifest_path")`.
#[test]
fn read_before_write_parses_from_toml() {
    let toml_str = r#"
        schema_version = "2.0"
        name        = "test-cap"
        version     = "1.0.0"
        namespace   = "test"
        kind        = "chain"
        description = "test"
        tags        = []
        accepts     = []
        emits       = []
        idempotent  = true
        requires    = []

        [[tools]]
        name               = "add_dep"
        description        = "add a dependency"
        read_before_write  = "manifest_path"

        [tools.input_schema]
        type = "object"
    "#;

    let manifest =
        ToolManifest::from_toml(toml_str).expect("manifest with read_before_write must parse");
    let tool = manifest.tools.first().expect("must have at least one tool");
    assert_eq!(
        tool.read_before_write.as_deref(),
        Some("manifest_path"),
        "read_before_write must deserialise from TOML"
    );
}

/// A TOML manifest whose tool omits `read_before_write` must deserialise to
/// `None` — the field defaults via `#[serde(default)]`.
#[test]
fn missing_read_before_write_defaults_to_none_toml() {
    let toml_str = r#"
        schema_version = "2.0"
        name        = "test-cap"
        version     = "1.0.0"
        namespace   = "test"
        kind        = "chain"
        description = "test"
        tags        = []
        accepts     = []
        emits       = []
        idempotent  = true
        requires    = []

        [[tools]]
        name        = "do_thing"
        description = "do it"

        [tools.input_schema]
        type = "object"
    "#;

    let manifest = ToolManifest::from_toml(toml_str).expect("should deserialise");
    let tool = manifest.tools.first().expect("must have a tool");
    assert!(
        tool.read_before_write.is_none(),
        "absent read_before_write must default to None; got {:?}",
        tool.read_before_write
    );
}

// ── 2.D.5b — JSON serde ──────────────────────────────────────────────────────

/// `read_before_write` must round-trip through JSON (used in API transport).
#[test]
fn read_before_write_survives_json_round_trip() {
    let tool = ToolDef {
        name: "add_dep".into(),
        description: "add a dependency".into(),
        input_schema: serde_json::json!({"type": "object"}),
        search_keywords: vec![],
        read_before_write: Some("manifest_path".into()),
    };

    let json = serde_json::to_string(&tool).expect("should serialise to JSON");
    assert!(
        json.contains("manifest_path"),
        "JSON must include read_before_write value; got: {json}"
    );

    let restored: ToolDef = serde_json::from_str(&json).expect("should deserialise from JSON");
    assert_eq!(
        restored.read_before_write.as_deref(),
        Some("manifest_path"),
        "read_before_write must survive JSON round-trip"
    );
}

/// A JSON object without `read_before_write` must deserialise to a `ToolDef`
/// with `read_before_write = None`.
#[test]
fn json_without_read_before_write_deserialises_to_none() {
    let json = r#"{
        "name": "tool",
        "description": "desc",
        "input_schema": {"type": "object"},
        "search_keywords": []
    }"#;
    let tool: ToolDef = serde_json::from_str(json).expect("should deserialise");
    assert!(
        tool.read_before_write.is_none(),
        "absent field must default to None; got {:?}",
        tool.read_before_write
    );
}

// ── 2.D.5c — live capabilities dir ───────────────────────────────────────────

/// When the `capabilities/` directory is present, the `code-project` manifest
/// must set `read_before_write = "manifest_path"` on the `add_dependency` tool.
#[test]
fn code_project_add_dependency_has_read_before_write() {
    let dir = capabilities_dir();
    if !dir.exists() {
        // CI environments without the capabilities tree skip this test.
        return;
    }

    let toml_path = dir.join("code-project").join("capability.toml");
    if !toml_path.exists() {
        return;
    }

    let contents = std::fs::read_to_string(&toml_path).expect("should read capability.toml");
    let manifest =
        ToolManifest::from_toml(&contents).expect("code-project capability.toml should parse");

    let add_dep = manifest
        .tools
        .iter()
        .find(|t| t.name == "add_dependency")
        .expect("code-project must have an add_dependency tool");

    assert_eq!(
        add_dep.read_before_write.as_deref(),
        Some("manifest_path"),
        "add_dependency tool must have read_before_write = \"manifest_path\""
    );
}
