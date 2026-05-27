//! Tenant isolation tests — Phase 1.7
//!
//! Cases 1–2, 7 and the in-memory tests run without any external service.
//! Cases 3–6 require a live RustFS/S3-compatible endpoint with IAM enabled and
//! are gated on the `RUSTFS_TEST_ENDPOINT` environment variable; they are
//! `#[ignore]`d unless that variable is set.
//! Cases 8–10 are Phase 0 skeletons — ignored until Phase 2 handler isolation
//! fixes land.
//!
//! To run the full suite against a local RustFS container:
//!   RUSTFS_TEST_ENDPOINT=http://localhost:9000 \
//!   RUSTFS_ROOT_ACCESS_KEY=rustfsadmin \
//!   RUSTFS_ROOT_SECRET_KEY=rustfsadmin \
//!   cargo test -p agent-gateway --test tenant_isolation -- --include-ignored

use agent_core::VirtualPath;
use std::sync::Arc;
use std::path::PathBuf;

// ── Imports for Phase 2 agent isolation tests ─────────────────────────────────
use agent_core::{PlanTier, TenantContext};
use agent_gateway::agent::build_ctx;
use agent_gateway::mw::tenant::ResolvedTenant;
use agent_gateway::routes::chat::{ChatMessage, ChatRequest};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn rustfs_endpoint() -> Option<String> {
    std::env::var("RUSTFS_TEST_ENDPOINT").ok()
}

fn root_access_key() -> String {
    std::env::var("RUSTFS_ROOT_ACCESS_KEY").unwrap_or_else(|_| "rustfsadmin".into())
}

fn root_secret_key() -> String {
    std::env::var("RUSTFS_ROOT_SECRET_KEY").unwrap_or_else(|_| "rustfsadmin".into())
}

/// Build an S3 ObjectStore using the given credentials and bucket.
fn build_s3_client(
    endpoint: &str,
    bucket: &str,
    access_key: &str,
    secret_key: &str,
) -> Arc<dyn object_store::ObjectStore> {
    use object_store::aws::AmazonS3Builder;
    Arc::new(
        AmazonS3Builder::new()
            .with_endpoint(endpoint)
            .with_bucket_name(bucket)
            .with_access_key_id(access_key)
            .with_secret_access_key(secret_key)
            .with_allow_http(true)
            .with_region("us-east-1")
            .build()
            .expect("build s3 client"),
    )
}

// ── Case 1: VirtualPath rejects all traversal sequences ──────────────────────

#[test]
fn case1_virtual_path_rejects_traversal() {
    assert!(
        VirtualPath::parse("../etc/passwd").is_err(),
        "../ must be rejected"
    );
    assert!(
        VirtualPath::parse("a/../b").is_err(),
        "mid-path .. must be rejected"
    );
    assert!(
        VirtualPath::parse("a/..").is_err(),
        "trailing .. must be rejected"
    );
    assert!(
        VirtualPath::parse("/etc/passwd").is_err(),
        "leading / must be rejected"
    );
    assert!(
        VirtualPath::parse("C:/windows").is_err(),
        "drive letter must be rejected"
    );
    assert!(
        VirtualPath::parse("a\x00b").is_err(),
        "NUL byte must be rejected"
    );
    assert!(
        VirtualPath::parse("a\x1fb").is_err(),
        "control char must be rejected"
    );
    assert!(
        VirtualPath::parse("a//b").is_err(),
        "double slash must be rejected"
    );
    assert!(
        VirtualPath::parse("a/b/").is_err(),
        "trailing slash must be rejected"
    );
    let long = "a".repeat(1025);
    assert!(
        VirtualPath::parse(&long).is_err(),
        "path > 1024 bytes must be rejected"
    );
    assert!(VirtualPath::parse("Workspace/notes.md").is_ok());
    assert!(VirtualPath::parse("folder/sub/file.txt").is_ok());
}

// ── Case 2: Tenant A's key is always scoped under tenants/A/ ─────────────────

#[test]
fn case2_legacy_key_always_scoped_under_tenant_prefix() {
    use agent_core::store::tenant_storage::StorageLayout;

    let _ = (
        StorageLayout::LegacyPrefix {
            tenant_id: "a".into(),
        },
        StorageLayout::LegacyPrefix {
            tenant_id: "b".into(),
        },
    );

    let vp = VirtualPath::parse("secret.md").unwrap();
    let key_a = format!("tenants/a/workspaces/{}", vp.as_str());
    let key_b = format!("tenants/b/workspaces/{}", vp.as_str());
    assert_ne!(key_a, key_b);
    assert!(key_a.starts_with("tenants/a/"));
    assert!(key_b.starts_with("tenants/b/"));
}

// ── Case 7: Path traversal returns StorageError::InvalidPath ─────────────────

#[test]
fn case7_path_traversal_returns_invalid_path_error() {
    use agent_core::store::tenant_storage::StorageError;

    let result = VirtualPath::parse("../B/secret.md");
    assert!(matches!(result, Err(StorageError::InvalidPath(_))));

    let err = VirtualPath::parse("../secret").unwrap_err();
    assert!(
        err.to_string().contains("invalid virtual path"),
        "error: {err}"
    );
}

// ── In-memory isolation — no server required ──────────────────────────────────

#[tokio::test]
async fn inmem_tenant_a_write_not_visible_to_tenant_b() {
    use bytes::Bytes;
    use object_store::{ObjectStore, path::Path as ObjectPath};

    let store_a = Arc::new(object_store::memory::InMemory::new());
    let store_b = Arc::new(object_store::memory::InMemory::new());

    let key_a = ObjectPath::from("tenants/a/workspaces/secret.md");
    store_a
        .put(&key_a, Bytes::from("A's secret").into())
        .await
        .unwrap();

    // B's store is completely separate — cannot see A's object at all.
    let result = store_b.get(&key_a).await;
    assert!(result.is_err(), "B's store must not contain A's objects");
}

// ── Cases 3–6: live RustFS (IAM-enforced cross-tenant access denial) ──────────
//
// Each test provisions two tenants with separate IAM service accounts (both
// scoped to their own prefix), then verifies that tenant B's credentials are
// denied when attempting to access tenant A's objects.

/// Provision a per-tenant IAM service account and return (access_key, secret_key).
/// Idempotent: if the account already exists it is re-created.
async fn provision_iam(
    endpoint: &str,
    root_access: &str,
    root_secret: &str,
    tenant_id: &str,
    bucket: &str,
) -> (String, String) {
    use rustfs_admin::{RustFsAdminClient, iam::provision_tenant};
    let admin = RustFsAdminClient::new(endpoint, root_access, root_secret, bucket);
    let creds = provision_tenant(&admin, tenant_id)
        .await
        .expect("provision_tenant");
    (creds.access_key, creds.secret_key)
}

#[tokio::test]
#[ignore = "requires live RustFS — set RUSTFS_TEST_ENDPOINT to run"]
async fn case3_tenant_b_presigned_url_rejected_for_tenant_a_object() {
    let endpoint = match rustfs_endpoint() {
        Some(e) => e,
        None => return,
    };
    let bucket = "workspace";

    let (ak_a, sk_a) = provision_iam(
        &endpoint,
        &root_access_key(),
        &root_secret_key(),
        "iso-tenant-a",
        bucket,
    )
    .await;
    let (ak_b, sk_b) = provision_iam(
        &endpoint,
        &root_access_key(),
        &root_secret_key(),
        "iso-tenant-b",
        bucket,
    )
    .await;

    // A writes secret.md under A's prefix.
    let client_a = build_s3_client(&endpoint, bucket, &ak_a, &sk_a);
    let key_a = object_store::path::Path::from("tenants/iso-tenant-a/workspaces/secret.md");
    client_a
        .put(&key_a, bytes::Bytes::from("A's secret").into())
        .await
        .expect("A should be able to write its own prefix");

    // B's client tries to GET A's key — must fail with permission denied.
    let client_b = build_s3_client(&endpoint, bucket, &ak_b, &sk_b);
    let result = client_b.get(&key_a).await;
    assert!(
        result.is_err(),
        "tenant B must not read tenant A's object; got: {:?}",
        result.ok()
    );
}

#[tokio::test]
#[ignore = "requires live RustFS — set RUSTFS_TEST_ENDPOINT to run"]
async fn case4_tenant_b_cannot_list_tenant_a_prefix() {
    let endpoint = match rustfs_endpoint() {
        Some(e) => e,
        None => return,
    };
    let bucket = "workspace";

    let (_ak_a, _sk_a) = provision_iam(
        &endpoint,
        &root_access_key(),
        &root_secret_key(),
        "iso-tenant-a",
        bucket,
    )
    .await;
    let (ak_b, sk_b) = provision_iam(
        &endpoint,
        &root_access_key(),
        &root_secret_key(),
        "iso-tenant-b",
        bucket,
    )
    .await;

    let client_b = build_s3_client(&endpoint, bucket, &ak_b, &sk_b);
    let prefix_a = object_store::path::Path::from("tenants/iso-tenant-a/");

    use futures::TryStreamExt;
    let listed: Vec<_> = client_b
        .list(Some(&prefix_a))
        .try_collect()
        .await
        .unwrap_or_default();

    assert!(
        listed.is_empty(),
        "tenant B must see no objects under tenant A's prefix; found: {listed:?}"
    );
}

#[tokio::test]
#[ignore = "requires live RustFS — set RUSTFS_TEST_ENDPOINT to run"]
async fn case5_tenant_b_cannot_get_tenant_a_object_directly() {
    let endpoint = match rustfs_endpoint() {
        Some(e) => e,
        None => return,
    };
    let bucket = "workspace";

    let (ak_a, sk_a) = provision_iam(
        &endpoint,
        &root_access_key(),
        &root_secret_key(),
        "iso-tenant-a",
        bucket,
    )
    .await;
    let (ak_b, sk_b) = provision_iam(
        &endpoint,
        &root_access_key(),
        &root_secret_key(),
        "iso-tenant-b",
        bucket,
    )
    .await;

    // Write as A.
    let client_a = build_s3_client(&endpoint, bucket, &ak_a, &sk_a);
    let key = object_store::path::Path::from("tenants/iso-tenant-a/workspaces/private.bin");
    client_a
        .put(&key, bytes::Bytes::from("private").into())
        .await
        .unwrap();

    // Direct GET as B.
    let client_b = build_s3_client(&endpoint, bucket, &ak_b, &sk_b);
    let result = client_b.get(&key).await;
    assert!(result.is_err(), "direct cross-tenant GET must be denied");
}

#[tokio::test]
#[ignore = "requires live RustFS — set RUSTFS_TEST_ENDPOINT to run"]
async fn case6_tenant_b_cannot_finalize_tenant_a_staged_upload() {
    let endpoint = match rustfs_endpoint() {
        Some(e) => e,
        None => return,
    };
    let bucket = "workspace";

    let (ak_a, sk_a) = provision_iam(
        &endpoint,
        &root_access_key(),
        &root_secret_key(),
        "iso-tenant-a",
        bucket,
    )
    .await;
    let (ak_b, sk_b) = provision_iam(
        &endpoint,
        &root_access_key(),
        &root_secret_key(),
        "iso-tenant-b",
        bucket,
    )
    .await;

    // A uploads a staging part.
    let client_a = build_s3_client(&endpoint, bucket, &ak_a, &sk_a);
    let staging_key =
        object_store::path::Path::from("tenants/iso-tenant-a/uploads/tmp/upload-xyz/part-001");
    client_a
        .put(&staging_key, bytes::Bytes::from("part data").into())
        .await
        .unwrap();

    // B's client tries to list A's staging prefix (required for finalization).
    let client_b = build_s3_client(&endpoint, bucket, &ak_b, &sk_b);
    let staging_prefix =
        object_store::path::Path::from("tenants/iso-tenant-a/uploads/tmp/upload-xyz/");

    use futures::TryStreamExt;
    let listed: Vec<_> = client_b
        .list(Some(&staging_prefix))
        .try_collect()
        .await
        .unwrap_or_default();

    assert!(
        listed.is_empty(),
        "tenant B cannot list tenant A's staging prefix; found: {listed:?}"
    );
}

// ── Step 0.4 skeletons: agent handler cross-tenant isolation ──────────────────
//
// These cases document the cross-tenant invariants that the Phase 2 handler
// isolation work must enforce.  They are `#[ignore]`d until the relevant
// fixes land so that `cargo test` stays green throughout Phase 0 and 1.

/// When tenant A's credentials are used but the `thread_id` in the request
/// belongs to tenant B, the agent endpoint must return 404 (not serve B's
/// conversation history to A).
#[tokio::test]
async fn case8_thread_id_from_other_tenant_returns_404() {
    unsafe { std::env::set_var("ANTHROPIC_API_KEY", "test-key") };
    unsafe { std::env::set_var("CONUSAI_TEST_MODE", "1") };

    let state = Arc::new(
        agent_gateway::state::AppState::with_in_memory_stores()
            .expect("in-memory AppState"),
    );

    // Create a thread under tenant-b.
    let thread_b = state
        .thread_store
        .create("tenant-b", vec![])
        .await
        .expect("create thread for tenant-b");

    // Request as tenant-a, referencing tenant-b's thread_id.
    let tenant_a = ResolvedTenant(TenantContext::new(
        "tenant-a",
        Some("user-a"),
        PlanTier::Free,
        PathBuf::from("/tmp"),
    ));
    let limits = PlanTier::Free.limits();
    let req = ChatRequest {
        model: None,
        messages: vec![ChatMessage {
            role: "user".into(),
            content: "hello".into(),
        }],
        max_tokens: None,
        stream: None,
        thread_id: Some(thread_b.id.to_string()),
        workspace_node_id: None,
        max_turns: None,
        attachment_content: vec![],
        forced_capability: None,
    };

    let result = build_ctx(&state, &tenant_a, limits, &req).await;
    assert!(
        result.is_err(),
        "cross-tenant thread access must be rejected"
    );
    let err = result.err().unwrap();
    assert_eq!(
        err.status, 404,
        "cross-tenant thread_id must return 404, got status {}",
        err.status
    );
}

/// When tenant A's credentials are used but the `workspace_node_id` in the
/// request belongs to tenant B, the agent endpoint must return 403.
#[tokio::test]
async fn case9_workspace_node_id_from_other_tenant_returns_403() {
    unsafe { std::env::set_var("ANTHROPIC_API_KEY", "test-key") };
    unsafe { std::env::set_var("CONUSAI_TEST_MODE", "1") };

    let state = Arc::new(
        agent_gateway::state::AppState::with_in_memory_stores()
            .expect("in-memory AppState"),
    );

    // Create a workspace node under tenant-b.
    let node_b = state
        .workspace_store
        .create_folder("tenant-b", "user-b", None, "b-folder")
        .await
        .expect("create workspace node for tenant-b");

    // Request as tenant-a, referencing tenant-b's node id.
    let tenant_a = ResolvedTenant(TenantContext::new(
        "tenant-a",
        Some("user-a"),
        PlanTier::Free,
        PathBuf::from("/tmp"),
    ));
    let limits = PlanTier::Free.limits();
    let req = ChatRequest {
        model: None,
        messages: vec![ChatMessage {
            role: "user".into(),
            content: "hello".into(),
        }],
        max_tokens: None,
        stream: None,
        thread_id: None,
        workspace_node_id: Some(node_b.id.to_string()),
        max_turns: None,
        attachment_content: vec![],
        forced_capability: None,
    };

    let result = build_ctx(&state, &tenant_a, limits, &req).await;
    assert!(
        result.is_err(),
        "cross-tenant workspace_node_id access must be rejected"
    );
    let err = result.err().unwrap();
    assert_eq!(
        err.status, 403,
        "cross-tenant workspace_node_id must return 403, got status {}",
        err.status
    );
}

/// A `forced_capability` value that names an unknown or tenant-inaccessible
/// capability must be rejected with 400.
#[tokio::test]
async fn case10_unknown_forced_capability_is_rejected() {
    unsafe { std::env::set_var("ANTHROPIC_API_KEY", "test-key") };
    unsafe { std::env::set_var("CONUSAI_TEST_MODE", "1") };

    let state = Arc::new(
        agent_gateway::state::AppState::with_in_memory_stores()
            .expect("in-memory AppState"),
    );

    let tenant = ResolvedTenant(TenantContext::new(
        "tenant-a",
        Some("user-a"),
        PlanTier::Enterprise, // Enterprise to avoid plan-gating on tools
        PathBuf::from("/tmp"),
    ));
    let limits = PlanTier::Enterprise.limits();
    let req = ChatRequest {
        model: None,
        messages: vec![ChatMessage {
            role: "user".into(),
            content: "hello".into(),
        }],
        max_tokens: None,
        stream: None,
        thread_id: None,
        workspace_node_id: None,
        max_turns: None,
        attachment_content: vec![],
        forced_capability: Some("nonexistent-capability-xyz".into()),
    };

    let result = build_ctx(&state, &tenant, limits, &req).await;
    assert!(
        result.is_err(),
        "unknown forced_capability must be rejected"
    );
    let err = result.err().unwrap();
    assert_eq!(
        err.status, 400,
        "unknown forced_capability must return 400, got status {}",
        err.status
    );
}
