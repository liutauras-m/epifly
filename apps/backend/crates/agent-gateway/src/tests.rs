//! Router-level regression tests (S1, S2, S5, S6, S9, S10, S13, `/metrics`).
//!
//! The router is built exactly as in production, over in-memory state, with a
//! test token verifier injected through the `TokenVerifier` trait.

use crate::auth::{hash_api_key, parse_api_keys};
use crate::routes::ROUTE_TABLE;
use crate::state::AppState;
use crate::state::test_support::{TEST_PLATFORM_TOKEN, state};
use agent_core::{AuthError, Identity, TokenVerifier, UserRole};
use async_trait::async_trait;
use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use common::types::TenantId;
use serde_json::{Value, json};
use std::sync::Arc;
use tower::ServiceExt;

/// `tok-<tenant>` → user of that tenant; `admin-<tenant>` → super_admin.
struct TestVerifier;

#[async_trait]
impl TokenVerifier for TestVerifier {
    async fn verify(&self, token: &str) -> Result<Identity, AuthError> {
        let (role, tenant) = if let Some(t) = token.strip_prefix("tok-") {
            (UserRole::User, t)
        } else if let Some(t) = token.strip_prefix("admin-") {
            (UserRole::SuperAdmin, t)
        } else {
            return Err(AuthError::InvalidToken("unknown test token".into()));
        };
        Ok(Identity {
            user_id: format!("user-{tenant}"),
            tenant_id: TenantId::from(tenant.to_string()),
            email: None,
            role,
        })
    }
}

const MACHINE_KEY: &str = "machine-key-for-org-k";

fn app_state() -> Arc<AppState> {
    let keys = parse_api_keys(&format!("{}:org-k:ci", hash_api_key(MACHINE_KEY))).unwrap();
    Arc::new(state(Arc::new(TestVerifier), keys))
}

fn router(state: &Arc<AppState>) -> Router {
    crate::build_router(
        Arc::clone(state),
        Arc::new(prometheus::Registry::new()),
        &["https://app.example".to_string()],
    )
}

async fn send(
    app: &Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(uri);
    if let Some(t) = token {
        req = req.header("authorization", format!("Bearer {t}"));
    }
    let req = match body {
        Some(b) => req
            .header("content-type", "application/json")
            .body(Body::from(b.to_string())),
        None => req.body(Body::empty()),
    }
    .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

fn concrete_path(path: &str) -> String {
    path.split('/')
        .map(|seg| {
            if seg.starts_with('{') {
                match seg {
                    "{id}" | "{upload_id}" => "01HZZZZZZZZZZZZZZZZZZZZZZZ",
                    "{n}" => "1",
                    _ => "x",
                }
            } else {
                seg
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[tokio::test]
async fn every_protected_and_admin_route_requires_a_bearer_token() {
    let state = app_state();
    let app = router(&state);
    for r in ROUTE_TABLE
        .iter()
        .filter(|r| matches!(r.auth, "bearer" | "super-admin"))
    {
        let uri = concrete_path(r.path);
        let body = (r.method != "GET" && r.method != "DELETE").then(|| json!({}));
        let (status, _) = send(&app, r.method, &uri, None, body.clone()).await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "{} {} without token",
            r.method,
            r.path
        );
        let (status, _) = send(&app, r.method, &uri, Some("forged.jwt.value"), body).await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "{} {} with a bad token",
            r.method,
            r.path
        );
    }
}

#[tokio::test]
async fn tenant_header_and_legacy_credentials_are_ignored() {
    let state = app_state();
    let app = router(&state);
    let req = Request::builder()
        .uri("/v1/tasks")
        .header("x-tenant-id", "org-a")
        .header("x-session-token", "anything")
        .header("cookie", "conusai_session=anything")
        .header("x-api-key", MACHINE_KEY)
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn api_key_authenticates_as_its_tenant() {
    let state = app_state();
    let app = router(&state);
    let (status, body) = send(&app, "GET", "/v1/capabilities", Some(MACHINE_KEY), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["tenant_id"], "org-k");
    assert_eq!(body["plan"], "free", "plan is resolved server-side");
}

#[tokio::test]
async fn s1_download_requires_auth_and_rejects_bad_keys() {
    let state = app_state();
    let app = router(&state);
    let (status, _) = send(
        &app,
        "GET",
        "/ui/files/download?key=tenants/org-b/u/x.png",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = send(
        &app,
        "GET",
        "/ui/files/download?key=../etc/passwd",
        Some("tok-org-a"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    // Without tenant storage there is no other path to the object (no root-key fallback).
    let (status, _) = send(
        &app,
        "GET",
        "/ui/files/download?key=tenants/org-b/u/x.png",
        Some("tok-org-a"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn s2_billing_dashboard_is_super_admin_only() {
    let state = app_state();
    let app = router(&state);
    let (status, _) = send(&app, "GET", "/ui/admin/billing", None, None).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "nothing under /ui answers without a token"
    );
    let (status, _) = send(&app, "GET", "/ui/admin/billing", Some("admin-org-a"), None).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "the HTML billing dashboard is gone"
    );
    let (status, _) = send(
        &app,
        "GET",
        "/admin/billing/dashboard",
        Some("tok-org-a"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = send(&app, "GET", "/admin/jobs", Some("admin-org-a"), None).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn deleted_legacy_routes_are_gone() {
    let state = app_state();
    let app = router(&state);
    for (m, p) in [
        ("POST", "/v1/auth/login"),
        ("POST", "/v1/auth/legacy/login"),
        ("GET", "/login"),
        ("POST", "/admin/devices"),
        ("GET", "/docs"),
    ] {
        let (status, _) = send(&app, m, p, Some("admin-org-a"), Some(json!({}))).await;
        assert!(
            status == StatusCode::NOT_FOUND || status == StatusCode::METHOD_NOT_ALLOWED,
            "{m} {p} → {status}"
        );
    }
}

#[tokio::test]
async fn s9_tasks_are_tenant_scoped() {
    let state = app_state();
    let app = router(&state);
    let id = state
        .job_executor
        .enqueue(
            "video-transcription",
            json!({"file_id": "k"}),
            Some("org-b"),
        )
        .await
        .unwrap();

    let (status, _) = send(
        &app,
        "GET",
        &format!("/v1/tasks/{id}"),
        Some("tok-org-a"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = send(
        &app,
        "GET",
        &format!("/v1/tasks/{id}/sse"),
        Some("tok-org-a"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, list) = send(&app, "GET", "/v1/tasks", Some("tok-org-a"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list, json!([]));

    let (status, task) = send(
        &app,
        "GET",
        &format!("/v1/tasks/{id}"),
        Some("tok-org-b"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(task["tenant_id"], "org-b");
}

#[tokio::test]
async fn s13_client_cannot_pick_a_model_outside_its_plan() {
    let state = app_state();
    let app = router(&state);
    let body = json!({"model": "opus", "messages": [{"role": "user", "content": "hi"}]});
    let (status, _) = send(
        &app,
        "POST",
        "/v1/chat/completions",
        Some("tok-org-a"),
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn metrics_require_the_platform_token() {
    let state = app_state();
    let app = router(&state);
    let (status, _) = send(&app, "GET", "/metrics", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = send(&app, "GET", "/metrics", Some("admin-org-a"), None).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "a user token is not the platform token"
    );
    let (status, _) = send(&app, "GET", "/metrics", Some(TEST_PLATFORM_TOKEN), None).await;
    assert_eq!(status, StatusCode::OK);
}

fn registration(tenant_scope: Value, global: bool) -> Value {
    json!({
        "capability_id": "acme.ocr",
        "namespace": "acme",
        "version": "1.0.0",
        "kind": "remote_mcp",
        "endpoint": "https://mcp.acme.test/rpc",
        "tenant_scope": tenant_scope,
        "global": global,
        "tools": [{
            "name": "ocr",
            "description": "Extract text",
            "input_schema": {"type": "object"}
        }]
    })
}

fn mcp_call(method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params})
}

/// S6 + S10 + S11 in one test: it is the only test in this binary that sets
/// `CONUSAI_MCP_ALLOWED_HOSTS`.
#[tokio::test]
async fn s6_s10_s11_registration_and_mcp_scoping() {
    let state = app_state();
    let app = router(&state);
    let reg = "/admin/capabilities/register";

    // No token, a user token, or a device-style header: refused.
    let (status, _) = send(
        &app,
        "POST",
        reg,
        None,
        Some(registration(json!(["org-a"]), false)),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = send(
        &app,
        "POST",
        reg,
        Some("admin-org-a"),
        Some(registration(json!(["org-a"]), false)),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Empty allowlist denies every remote endpoint.
    // SAFETY: only this test touches the variable.
    unsafe { std::env::remove_var("CONUSAI_MCP_ALLOWED_HOSTS") };
    let (status, _) = send(
        &app,
        "POST",
        reg,
        Some(TEST_PLATFORM_TOKEN),
        Some(registration(json!(["org-a"]), false)),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    // Exact-host match only: a suffix trick is refused.
    unsafe { std::env::set_var("CONUSAI_MCP_ALLOWED_HOSTS", "acme.test") };
    let (status, _) = send(
        &app,
        "POST",
        reg,
        Some(TEST_PLATFORM_TOKEN),
        Some(registration(json!(["org-a"]), false)),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "mcp.acme.test is not acme.test"
    );
    unsafe { std::env::set_var("CONUSAI_MCP_ALLOWED_HOSTS", "mcp.acme.test") };

    // Tenant-scoped by default; global must be explicit.
    let (status, _) = send(
        &app,
        "POST",
        reg,
        Some(TEST_PLATFORM_TOKEN),
        Some(registration(json!([]), false)),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (status, _) = send(
        &app,
        "POST",
        reg,
        Some(TEST_PLATFORM_TOKEN),
        Some(registration(json!(["org-a"]), false)),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // Never overwrite an existing capability.
    let (status, _) = send(
        &app,
        "POST",
        reg,
        Some(TEST_PLATFORM_TOKEN),
        Some(registration(json!(["org-a"]), false)),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // S10: tools/list and tools/call honour tenant_scope.
    let (_, listed_a) = send(
        &app,
        "POST",
        "/mcp",
        Some("tok-org-a"),
        Some(mcp_call("tools/list", json!({}))),
    )
    .await;
    let names_a = listed_a.to_string();
    assert!(
        names_a.contains("acme_ocr__ocr") || names_a.contains("acme.ocr__ocr"),
        "{names_a}"
    );
    let (_, listed_b) = send(
        &app,
        "POST",
        "/mcp",
        Some("tok-org-b"),
        Some(mcp_call("tools/list", json!({}))),
    )
    .await;
    assert!(
        !listed_b.to_string().contains("acme"),
        "org-b must not see org-a's capability"
    );
    let (_, called_b) = send(
        &app,
        "POST",
        "/mcp",
        Some("tok-org-b"),
        Some(mcp_call(
            "tools/call",
            json!({"name": "acme_ocr__ocr", "arguments": {}}),
        )),
    )
    .await;
    assert!(
        called_b["error"]["message"]
            .as_str()
            .unwrap_or("")
            .contains("not found"),
        "{called_b}"
    );

    // S10: a disabled capability is neither listed nor callable.
    state
        .registry
        .lock()
        .unwrap()
        .set_enabled("acme.ocr", false);
    let (_, listed_a) = send(
        &app,
        "POST",
        "/mcp",
        Some("tok-org-a"),
        Some(mcp_call("tools/list", json!({}))),
    )
    .await;
    assert!(!listed_a.to_string().contains("acme"));
    let (_, called_a) = send(
        &app,
        "POST",
        "/mcp",
        Some("tok-org-a"),
        Some(mcp_call(
            "tools/call",
            json!({"name": "acme_ocr__ocr", "arguments": {}}),
        )),
    )
    .await;
    assert!(
        called_a["error"]["message"]
            .as_str()
            .unwrap_or("")
            .contains("not found"),
        "{called_a}"
    );

    unsafe { std::env::remove_var("CONUSAI_MCP_ALLOWED_HOSTS") };
}

#[tokio::test]
async fn cors_allows_web_and_native_shell_origins_only() {
    let state = app_state();
    let mut origins = vec!["https://app.example".to_string()];
    origins.extend(
        crate::config::NATIVE_SHELL_ORIGINS
            .iter()
            .map(|o| o.to_string()),
    );
    let app = crate::build_router(
        Arc::clone(&state),
        Arc::new(prometheus::Registry::new()),
        &origins,
    );
    for (origin, allowed) in [
        ("https://app.example", true),
        ("tauri://localhost", true),
        ("http://tauri.localhost", true),
        ("https://evil.example", false),
    ] {
        let req = Request::builder()
            .method("OPTIONS")
            .uri("/v1/tasks")
            .header("origin", origin)
            .header("access-control-request-method", "GET")
            .header("access-control-request-headers", "authorization")
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        let echoed = resp
            .headers()
            .get("access-control-allow-origin")
            .and_then(|v| v.to_str().ok())
            == Some(origin);
        assert_eq!(echoed, allowed, "{origin}");
    }
}
