//! Presign path-confusion regression tests — Phase 0, Step 0.3
//!
//! These tests were added as ignored RED regressions in Phase 0 and enabled in
//! Step 1.1 after the path-validation and containment fixes landed.
//!
//! Attack surface covered:
//!   A. Absolute / drive-letter paths     → already rejected by parse (green)
//!   B. Literal `..` traversal            → already rejected by parse (green)
//!   C. Percent-encoded traversal         → NOT yet rejected (red — Step 1.1)
//!   D. Sibling-prefix containment check  → NOT yet enforced (red — Step 1.1)

use agent_core::store::tenant_storage::{StorageError, VirtualPath};

// ── A. Absolute paths already rejected ────────────────────────────────────────

/// Leading `/` is already rejected by `VirtualPath::parse`.
/// An attacker cannot pass `"/tenants/other/secret.txt"` as a presign path.
#[test]
fn parse_rejects_absolute_path() {
    let result = VirtualPath::parse("/tenants/other/secret.txt");
    assert!(
        matches!(result, Err(StorageError::InvalidPath(_))),
        "absolute path must be rejected; got: {result:?}"
    );
}

/// Windows drive-letter paths are also rejected.
#[test]
fn parse_rejects_drive_letter_path() {
    let result = VirtualPath::parse("C:/Windows/secret");
    assert!(
        matches!(result, Err(StorageError::InvalidPath(_))),
        "drive-letter path must be rejected; got: {result:?}"
    );
}

// ── B. Literal `..` traversal already rejected ────────────────────────────────

/// `../` traversal in first segment is already rejected.
#[test]
fn parse_rejects_leading_dotdot() {
    let result = VirtualPath::parse("../secret.txt");
    assert!(
        matches!(result, Err(StorageError::InvalidPath(_))),
        "../ traversal must be rejected; got: {result:?}"
    );
}

/// Mid-path `..` traversal is also rejected.
#[test]
fn parse_rejects_mid_path_dotdot() {
    let result = VirtualPath::parse("folder/../secret.txt");
    assert!(
        matches!(result, Err(StorageError::InvalidPath(_))),
        "mid-path .. traversal must be rejected; got: {result:?}"
    );
}

/// Double-slash (empty segment) is also rejected.
#[test]
fn parse_rejects_double_slash() {
    let result = VirtualPath::parse("folder//secret.txt");
    assert!(
        matches!(result, Err(StorageError::InvalidPath(_))),
        "double slash must be rejected; got: {result:?}"
    );
}

// ── C. Percent-encoded traversal ─────────────────────────────────────────────

/// `VirtualPath::parse` decodes percent-encoded characters before checking for
/// path-traversal sequences.
///
/// `%2f` is URL-encoded `/` and `%2e` is URL-encoded `.`, so
/// `"a%2f..%2fetc"` decodes to `"a/../etc"` — a traversal attack.
#[test]
fn parse_rejects_percent_encoded_traversal() {
    // %2f = '/', %2e = '.' — decodes to "a/../etc"
    let result = VirtualPath::parse("a%2f..%2fetc");
    assert!(
        matches!(result, Err(StorageError::InvalidPath(_))),
        "percent-encoded traversal must be rejected; current code incorrectly accepts it"
    );
}

/// `%2e%2e` decodes to `..` — a traversal segment once URL-decoded.
#[test]
fn parse_rejects_percent_encoded_dotdot_segment() {
    let result = VirtualPath::parse("%2e%2e/secret.txt");
    assert!(
        matches!(result, Err(StorageError::InvalidPath(_))),
        "%2e%2e/secret must be rejected after decoding; current code accepts it"
    );
}

// ── D. Sibling-prefix containment ────────────────────────────────────────────

/// **Sibling-prefix attack**: `folder/foobar` starts with `folder/foo` as a
/// string prefix, but is NOT a child of `folder/foo`.  A naive
/// `starts_with` check on the raw path string would incorrectly allow this.
///
/// `VirtualPath::is_strict_child_of` enforces component-wise containment.
#[test]
fn sibling_prefix_is_not_a_child_path() {
    let node_vp = VirtualPath::parse("folder/foo").unwrap();
    let req_vp = VirtualPath::parse("folder/foobar").unwrap();
    assert!(
        !req_vp.is_strict_child_of(&node_vp),
        "folder/foobar is a sibling, not a child of folder/foo"
    );
}

/// A presign handler must verify that the requested `virtual_path` is
/// contained within the workspace node's own path — not merely that it
/// is a syntactically valid `VirtualPath`.
///
/// In the current code `presign_upload` and `presign_download` in
/// `workspaces.rs` call `VirtualPath::parse` but do **not** check
/// containment.  Step 1.1 adds that check.
#[test]
fn presign_handler_rejects_out_of_node_path() {
    let node_vp = VirtualPath::parse("project/docs").unwrap();
    let sibling = VirtualPath::parse("project/other-file.txt").unwrap();
    assert!(
        !sibling.is_strict_child_of(&node_vp),
        "presign handlers must reject paths outside the accessible node"
    );
}
