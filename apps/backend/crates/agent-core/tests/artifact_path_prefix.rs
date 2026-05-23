//! ArtifactBridge path-prefix plumbing tests (PR 2.C.6).
//!
//! Verifies that `process_if_artifacts` correctly routes artifact virtual paths
//! according to the `artifact_path_prefix` field in `ToolOutput.metadata`:
//!
//! | `artifact_path_prefix` value | Expected virtual path |
//! |---|---|
//! | absent / null | `/outputs/{tool_name}/{artifact.name}` (legacy default) |
//! | `""` (empty string) | `/{artifact.name}` — raw workspace path (code-project convention) |
//! | `"projects/foo"` | `projects/foo/{artifact.name}` |
//!
//! Object-store uploads are exercised only lightly (put succeeds) since the
//! meaningful contract here is the `WorkspaceContentStore` virtual path.

use agent_core::bridge::ArtifactBridge;
use base64::Engine as _;
use common::artifact::{Artifact, ToolOutput};
use common::memory::{InMemoryWorkspaceContent, WorkspaceContentStore};
use object_store::memory::InMemory;
use std::sync::Arc;

// ── helpers ───────────────────────────────────────────────────────────────────

fn bridge_with_inmem() -> (Arc<ArtifactBridge>, Arc<InMemoryWorkspaceContent>) {
    let store = Arc::new(InMemory::new());
    let content = Arc::new(InMemoryWorkspaceContent::new());
    let bridge = ArtifactBridge::new(store, Arc::clone(&content) as Arc<dyn WorkspaceContentStore>);
    (bridge, content)
}

fn text_artifact(name: &str, data: &str) -> Artifact {
    Artifact {
        name: name.to_string(),
        mime_type: "text/plain".to_string(),
        data: Some(data.to_string()),
        source_url: None,
        metadata: serde_json::Value::Null,
    }
}

fn output_with_prefix(artifacts: Vec<Artifact>, path_prefix: Option<&str>) -> ToolOutput {
    let mut meta = serde_json::json!({});
    if let Some(pfx) = path_prefix {
        meta["artifact_path_prefix"] = serde_json::json!(pfx);
    }
    ToolOutput {
        content: "ok".to_string(),
        artifacts,
        metadata: meta,
    }
}

const TENANT: &str = "test-tenant";
const TOOL: &str = "test_tool";

// ── 2.C tests ─────────────────────────────────────────────────────────────────

/// With `artifact_path_prefix = "projects/foo"`, artifact lands at
/// `projects/foo/<name>` in the content store.
#[tokio::test]
async fn explicit_prefix_routes_to_correct_path() {
    let (bridge, content) = bridge_with_inmem();
    let output = output_with_prefix(
        vec![text_artifact("README.md", "# Hello")],
        Some("projects/foo"),
    );

    let result = bridge
        .process_if_artifacts(TENANT, None, TOOL, None, &output)
        .await;
    assert!(result.is_ok(), "process_if_artifacts failed: {:?}", result);

    // Verify the content store has the artifact at the expected virtual path.
    let stored = content
        .read(TENANT, "projects/foo/README.md")
        .await
        .expect("should be readable at projects/foo/README.md");
    assert_eq!(stored, "# Hello");
}

/// With `artifact_path_prefix = ""` (empty string — code-project convention),
/// artifact lands at `/<name>`, i.e. the workspace root-relative path.
#[tokio::test]
async fn empty_prefix_routes_to_raw_path() {
    let (bridge, content) = bridge_with_inmem();
    let output = output_with_prefix(
        vec![text_artifact("projects/my-app/src/main.ts", "export {};\n")],
        Some(""),
    );

    let result = bridge
        .process_if_artifacts(TENANT, None, TOOL, None, &output)
        .await;
    assert!(result.is_ok(), "process_if_artifacts failed: {:?}", result);

    let stored = content
        .read(TENANT, "/projects/my-app/src/main.ts")
        .await
        .expect("should be readable at /projects/my-app/src/main.ts");
    assert_eq!(stored, "export {};\n");
}

/// Without any `artifact_path_prefix` in metadata, the legacy default path
/// `/outputs/{tool_name}/{artifact.name}` is used.
#[tokio::test]
async fn absent_prefix_uses_legacy_outputs_path() {
    let (bridge, content) = bridge_with_inmem();
    let output = output_with_prefix(
        vec![text_artifact("result.txt", "some output")],
        None, // no prefix key in metadata
    );

    let result = bridge
        .process_if_artifacts(TENANT, None, TOOL, None, &output)
        .await;
    assert!(result.is_ok(), "process_if_artifacts failed: {:?}", result);

    let expected_path = format!("/outputs/{TOOL}/result.txt");
    let stored = content
        .read(TENANT, &expected_path)
        .await
        .expect(&format!("should be readable at {expected_path}"));
    assert_eq!(stored, "some output");
}

/// Multiple artifacts in a single output all respect the same prefix.
#[tokio::test]
async fn multiple_artifacts_all_use_prefix() {
    let (bridge, content) = bridge_with_inmem();
    let output = output_with_prefix(
        vec![
            text_artifact("src/index.ts", "const x = 1;"),
            text_artifact("package.json", r#"{"name":"demo"}"#),
        ],
        Some("projects/demo"),
    );

    bridge
        .process_if_artifacts(TENANT, None, TOOL, None, &output)
        .await
        .expect("process_if_artifacts should succeed");

    assert_eq!(
        content.read(TENANT, "projects/demo/src/index.ts").await.unwrap(),
        "const x = 1;"
    );
    assert_eq!(
        content.read(TENANT, "projects/demo/package.json").await.unwrap(),
        r#"{"name":"demo"}"#
    );
}

/// Non-text artifacts (binary MIME) are uploaded to object store but NOT written
/// to the content store — the test just verifies the call succeeds without panic.
#[tokio::test]
async fn binary_artifact_does_not_write_to_content_store() {
    let (bridge, content) = bridge_with_inmem();
    let png_artifact = Artifact {
        name: "logo.png".to_string(),
        mime_type: "image/png".to_string(),
        data: Some(base64::engine::general_purpose::STANDARD.encode(b"\x89PNG")),
        source_url: None,
        metadata: serde_json::Value::Null,
    };
    let output = output_with_prefix(vec![png_artifact], Some("assets"));

    bridge
        .process_if_artifacts(TENANT, None, TOOL, None, &output)
        .await
        .expect("binary artifact should not fail process_if_artifacts");

    // Binary artifacts are NOT written to the workspace content store
    // (only text/* and application/json are indexed).  InMemoryWorkspaceContent
    // returns Ok("") for absent keys, so we check for the empty sentinel.
    let stored = content.read(TENANT, "assets/logo.png").await.unwrap_or_default();
    assert!(
        stored.is_empty(),
        "binary artifact must not appear in content store, got: {stored:?}"
    );
}
