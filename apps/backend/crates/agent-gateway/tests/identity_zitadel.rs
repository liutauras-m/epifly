//! Zitadel identity provider integration tests.
//!
//! Uses `wiremock` to stub `/oauth/v2/introspect` and asserts:
//! - First call performs a remote introspection (cache miss).
//! - Identical token within the TTL window is served from cache (cache hit, no extra request).
//! - Inactive token returns `AuthError::TokenExpired`.
//! - Missing `sub` claim returns `AuthError::InvalidToken`.

use agent_core::identity::zitadel::{VerifyMode, ZitadelConfig, ZitadelProvider};
use agent_core::identity::{AuthError, IdentityProvider};
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn make_provider(server: &MockServer) -> ZitadelProvider {
    ZitadelProvider::new(ZitadelConfig {
        issuer: server.uri(),
        domain: server.uri(),
        audience: "test-audience".into(),
        verify_mode: VerifyMode::Introspection,
        org_id_claim: "urn:zitadel:iam:user:resourceowner:id".into(),
        roles_claim: "urn:zitadel:iam:org:project:roles".into(),
        introspection_client_id: "client".into(),
        introspection_client_secret: "secret".into(),
        mgmt_pat: "pat".into(),
        is_dev: true,
    })
}

fn active_response(sub: &str, tenant_id: &str) -> serde_json::Value {
    serde_json::json!({
        "active": true,
        "sub": sub,
        "exp": 9_999_999_999u64,
        "email": format!("{sub}@example.com"),
        "urn:zitadel:iam:user:resourceowner:id": tenant_id,
        "urn:conusai:plan_tier": "pro",
        "urn:conusai:subscription_status": "active"
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn cache_miss_on_first_call_then_hit() {
    let server = MockServer::start().await;

    // Introspect endpoint — mounted once; it should only be hit once.
    Mock::given(method("POST"))
        .and(path("/oauth/v2/introspect"))
        .and(body_string_contains("token=test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(active_response("user1", "tenant1")))
        .expect(1) // asserts exactly one HTTP call is made
        .mount(&server)
        .await;

    let provider = make_provider(&server);

    // First call — must hit the wiremock server (miss).
    let ctx1 = provider
        .verify_access_token("test-token")
        .await
        .expect("first call failed");
    assert_eq!(ctx1.user_id, "user1");
    assert_eq!(ctx1.tenant_id.as_ref(), "tenant1");
    assert_eq!(provider.stats.misses(), 1, "expected 1 cache miss");
    assert_eq!(provider.stats.hits(), 0, "expected 0 cache hits");

    // Second call with the same token — must come from cache (no new HTTP call).
    let ctx2 = provider
        .verify_access_token("test-token")
        .await
        .expect("second call failed");
    assert_eq!(ctx2.user_id, "user1");
    assert_eq!(provider.stats.hits(), 1, "expected 1 cache hit");
    assert_eq!(provider.stats.misses(), 1, "still only 1 miss");

    // wiremock verifies `expect(1)` on drop — if a second HTTP call was made the test fails.
}

#[tokio::test]
async fn different_tokens_each_miss() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/oauth/v2/introspect"))
        .respond_with(ResponseTemplate::new(200).set_body_json(active_response("userA", "tenantA")))
        .mount(&server)
        .await;

    let provider = make_provider(&server);

    provider
        .verify_access_token("token-A")
        .await
        .expect("A failed");
    provider
        .verify_access_token("token-B")
        .await
        .expect("B failed");

    assert_eq!(
        provider.stats.misses(),
        2,
        "two distinct tokens = two misses"
    );
    assert_eq!(provider.stats.hits(), 0);
}

#[tokio::test]
async fn inactive_token_returns_expired_error() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/oauth/v2/introspect"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "active": false
        })))
        .mount(&server)
        .await;

    let provider = make_provider(&server);
    let err = provider
        .verify_access_token("dead-token")
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::TokenExpired), "got: {err:?}");
}

#[tokio::test]
async fn missing_sub_returns_invalid_token() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/oauth/v2/introspect"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "active": true,
            "exp": 9_999_999_999u64
            // no "sub"
        })))
        .mount(&server)
        .await;

    let provider = make_provider(&server);
    let err = provider
        .verify_access_token("no-sub-token")
        .await
        .unwrap_err();
    assert!(matches!(err, AuthError::InvalidToken(_)), "got: {err:?}");
}

#[tokio::test]
async fn introspect_http_failure_returns_provider_error() {
    // Point at a server that returns 500.
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/oauth/v2/introspect"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let provider = make_provider(&server);
    let err = provider
        .verify_access_token("some-token")
        .await
        .unwrap_err();
    assert!(
        matches!(err, AuthError::InvalidToken(_)),
        "HTTP 500 should map to InvalidToken, got: {err:?}"
    );
}
