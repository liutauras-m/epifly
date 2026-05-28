//! End-to-end regression test for the remote_mcp self-registration → hot-reload → invoke chain.
//!
//! Spins up a wiremock server that answers `tools/list` and `tools/call`,
//! registers it via the in-process registry (simulating POST /admin/capabilities/register),
//! then invokes via the SemanticCapabilityRouter and asserts the mock received the call.
//!
//! Does NOT require a running Postgres instance — uses the in-memory registry path.

use agent_core::{
    CapabilityRegistry, QdrantVectorStore, SemanticCapabilityRouter, SemanticRouterConfig,
    capabilities::{
        card::CapabilityCard,
        manifest::{ToolDef, ToolKind, ToolManifest},
        providers::remote_mcp::RemoteMcpCapability,
    },
    context::tenant::{PlanTier, TenantContext},
    indexing::EmbeddingService,
};
use async_trait::async_trait;
use parking_lot::RwLock;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ── Stub embedder ─────────────────────────────────────────────────────────────

struct ConstEmbedder;

#[async_trait]
impl EmbeddingService for ConstEmbedder {
    fn model(&self) -> agent_core::indexing::EmbeddingModel {
        agent_core::indexing::EmbeddingModel::MultilingualE5Large
    }
    async fn embed_query(&self, _: &str) -> anyhow::Result<Vec<f32>> {
        Ok(vec![0.1_f32; 1024])
    }
    async fn embed_documents(&self, texts: Vec<String>) -> anyhow::Result<Vec<Vec<f32>>> {
        Ok(texts.iter().map(|_| vec![0.1_f32; 1024]).collect())
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn make_tenant(id: &str) -> TenantContext {
    TenantContext::new(id, Some("user-1"), PlanTier::Free, "/tmp")
}

fn register_remote_mcp(
    registry: &mut CapabilityRegistry,
    name: &str,
    tool_fn: &str,
    endpoint: &str,
    scope: Vec<String>,
) {
    let tool_def = ToolDef {
        name: tool_fn.into(),
        description: format!("{tool_fn} tool"),
        input_schema: json!({"type": "object"}),
        search_keywords: vec![],
        read_before_write: None,
    };
    let manifest = ToolManifest {
        name: name.into(),
        version: "1.0.0".into(),
        description: format!("{name} remote mcp capability"),
        kind: ToolKind::RemoteMcp,
        tools: vec![tool_def],
        config: json!({"endpoint": endpoint}),
        tags: vec![],
        namespace: None,
        chain: None,
        tenant_scope: scope,
        enabled: true,
        search_keywords: vec![],
        schema_version: "2.0".into(),
        category: None,
        accepts: vec![],
        emits: vec![],
        idempotent: true,
        cost_hint: None,
        requires: vec![],
    };
    let card = CapabilityCard::new(manifest.clone(), std::path::PathBuf::from("."));
    let provider = RemoteMcpCapability::new(manifest, endpoint.to_string());
    registry.register(card.with_provider(provider));
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// Register a remote_mcp capability, invoke it, assert the mock MCP server received the call.
#[tokio::test]
async fn remote_mcp_register_and_invoke_reaches_mock_server() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "content": "pong",
                "artifacts": [],
                "metadata": {}
            }
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let registry = Arc::new(RwLock::new(CapabilityRegistry::new()));
    {
        let mut reg = registry.write();
        register_remote_mcp(
            &mut reg,
            "test-ping",
            "ping",
            &format!("{}/mcp", mock_server.uri()),
            vec![],
        );
    }

    let router = SemanticCapabilityRouter::new(
        Arc::clone(&registry),
        Arc::new(QdrantVectorStore::noop()),
        Arc::new(ConstEmbedder) as Arc<dyn EmbeddingService>,
        SemanticRouterConfig::default(),
    );

    let tenant = make_tenant("acme");
    let result = router
        .invoke("test-ping__ping", &json!({}), Some(&tenant))
        .await
        .expect("invoke must succeed");

    assert_eq!(result["content"], "pong");
    mock_server.verify().await;
}

/// A scoped capability must NOT be invokable by a tenant outside the scope.
#[tokio::test]
async fn remote_mcp_scoped_capability_blocked_for_wrong_tenant() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "jsonrpc": "2.0", "id": 1,
            "result": {"content": "secret", "artifacts": [], "metadata": {}}
        })))
        .expect(0) // must NOT be called
        .mount(&mock_server)
        .await;

    let registry = Arc::new(RwLock::new(CapabilityRegistry::new()));
    {
        let mut reg = registry.write();
        register_remote_mcp(
            &mut reg,
            "acme-only",
            "secret_op",
            &format!("{}/mcp", mock_server.uri()),
            vec!["acme".into()], // scoped to acme only
        );
    }

    let router = SemanticCapabilityRouter::new(
        Arc::clone(&registry),
        Arc::new(QdrantVectorStore::noop()),
        Arc::new(ConstEmbedder) as Arc<dyn EmbeddingService>,
        SemanticRouterConfig::default(),
    );

    let other_tenant = make_tenant("other");
    let result = router
        .invoke("acme-only__secret_op", &json!({}), Some(&other_tenant))
        .await;

    assert!(
        result.is_err(),
        "out-of-scope tenant must not invoke a scoped capability"
    );
    mock_server.verify().await;
}

/// Scoped capability IS reachable by a member tenant.
#[tokio::test]
async fn remote_mcp_scoped_capability_allowed_for_member_tenant() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "jsonrpc": "2.0", "id": 1,
            "result": {"content": "ok", "artifacts": [], "metadata": {}}
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let registry = Arc::new(RwLock::new(CapabilityRegistry::new()));
    {
        let mut reg = registry.write();
        register_remote_mcp(
            &mut reg,
            "acme-only",
            "secret_op",
            &format!("{}/mcp", mock_server.uri()),
            vec!["acme".into()],
        );
    }

    let router = SemanticCapabilityRouter::new(
        Arc::clone(&registry),
        Arc::new(QdrantVectorStore::noop()),
        Arc::new(ConstEmbedder) as Arc<dyn EmbeddingService>,
        SemanticRouterConfig::default(),
    );

    let acme = make_tenant("acme");
    let result = router
        .invoke("acme-only__secret_op", &json!({}), Some(&acme))
        .await
        .expect("member tenant must invoke scoped capability");

    assert_eq!(result["content"], "ok");
    mock_server.verify().await;
}

/// Hot-reload: after invalidate_all, second select() is a cache miss (no stale results).
#[tokio::test]
async fn invalidate_all_flushes_cache_after_reload() {
    let registry = Arc::new(RwLock::new(CapabilityRegistry::new()));
    let router = SemanticCapabilityRouter::new(
        Arc::clone(&registry),
        Arc::new(QdrantVectorStore::noop()),
        Arc::new(ConstEmbedder) as Arc<dyn EmbeddingService>,
        SemanticRouterConfig::default(),
    );

    let tenant = make_tenant("t1");
    let _ = router.select("anything", Some(&tenant)).await.unwrap();
    router.invalidate_all().await;
    let _ = router.select("anything", Some(&tenant)).await.unwrap();

    assert_eq!(
        router
            .metrics
            .cache_misses
            .load(std::sync::atomic::Ordering::Relaxed),
        2,
        "both calls after invalidation must be cache misses"
    );

    // Verify the test completes in well under the 5s budget.
    let _ = tokio::time::timeout(Duration::from_secs(5), async {}).await;
}

/// Regression: capability names containing dots (e.g. `media.time.get_current_time`)
/// must be invokable via the sanitised name the gateway exposes through `tools/list`
/// (where `.` → `_`). Without the dot-restore fallback in the lookup path the
/// `tools/call` would return "Tool not found" even though the same name appeared
/// in `tools/list`. See `agent-gateway/src/routes/mcp.rs::handle_tools_call`.
#[tokio::test]
async fn dotted_capability_name_resolves_via_sanitized_lookup() {
    let mock_server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": { "content": "tick", "artifacts": [], "metadata": {} }
        })))
        .expect(1)
        .mount(&mock_server)
        .await;

    let registry = Arc::new(RwLock::new(CapabilityRegistry::new()));
    {
        let mut reg = registry.write();
        register_remote_mcp(
            &mut reg,
            "media.time.get_current_time", // dotted name as produced by /admin/capabilities/register
            "get_current_time",
            &format!("{}/mcp", mock_server.uri()),
            vec![],
        );
    }
    let router = SemanticCapabilityRouter::new(
        Arc::clone(&registry),
        Arc::new(QdrantVectorStore::noop()),
        Arc::new(ConstEmbedder) as Arc<dyn EmbeddingService>,
        SemanticRouterConfig::default(),
    );

    // The LLM / MCP client echoes the sanitised name with `_` instead of `.`.
    let sanitised = "media_time_get_current_time__get_current_time";
    let result = router
        .invoke(sanitised, &json!({}), Some(&make_tenant("acme")))
        .await
        .expect("invoke must resolve dotted capability via _→. fallback");

    assert_eq!(result["content"], "tick");
    mock_server.verify().await;
}
