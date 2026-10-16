//! Zitadel token verification against a stubbed issuer (wiremock on loopback).
//!
//! JWT path: discovery → JWKS → RS256 signature, issuer, audience, expiry, org claim.
//! Opaque path: introspection with caching; inactive/foreign tokens are refused and
//! an unreachable issuer is reported as `Unavailable`, not as a valid identity.

use agent_core::identity::zitadel::{IntrospectionClient, ORG_CLAIM, ROLES_CLAIM};
use agent_core::{AuthError, TokenVerifier, UserRole, ZitadelConfig, ZitadelVerifier};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use rsa::pkcs1::EncodeRsaPrivateKey;
use rsa::traits::PublicKeyParts;
use serde_json::json;
use std::sync::OnceLock;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const AUD: &str = "gateway-project";

struct Key {
    pem: String,
    n: String,
    e: String,
}

/// Generated per test process; no key material is committed.
fn key() -> &'static Key {
    static K: OnceLock<Key> = OnceLock::new();
    K.get_or_init(|| {
        let private = rsa::RsaPrivateKey::new(&mut rand::thread_rng(), 2048).unwrap();
        Key {
            pem: private
                .to_pkcs1_pem(rsa::pkcs8::LineEnding::LF)
                .unwrap()
                .to_string(),
            n: URL_SAFE_NO_PAD.encode(private.n().to_bytes_be()),
            e: URL_SAFE_NO_PAD.encode(private.e().to_bytes_be()),
        }
    })
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

async fn issuer() -> MockServer {
    let server = MockServer::start().await;
    let uri = server.uri();
    Mock::given(method("GET"))
        .and(path("/.well-known/openid-configuration"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issuer": uri,
            "jwks_uri": format!("{uri}/oauth/v2/keys"),
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/oauth/v2/keys"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "keys": [{ "kty": "RSA", "kid": "k1", "alg": "RS256", "use": "sig", "n": key().n, "e": key().e }]
        })))
        .mount(&server)
        .await;
    server
}

fn verifier(server: &MockServer, introspection: bool) -> ZitadelVerifier {
    ZitadelVerifier::new(ZitadelConfig {
        issuer: server.uri(),
        audience: AUD.into(),
        introspection: introspection.then(|| IntrospectionClient {
            client_id: "client".into(),
            client_secret: "secret".into(),
        }),
    })
    .unwrap()
}

fn sign(iss: &str, extra: serde_json::Value) -> String {
    let mut claims = json!({
        "iss": iss, "sub": "user-1", "aud": [AUD], "exp": now() + 600, "iat": now(),
        ORG_CLAIM: "org-1", ROLES_CLAIM: { "super_admin": {} }
    });
    for (k, v) in extra.as_object().unwrap() {
        claims[k] = v.clone();
    }
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("k1".into());
    encode(
        &header,
        &claims,
        &EncodingKey::from_rsa_pem(key().pem.as_bytes()).unwrap(),
    )
    .unwrap()
}

#[tokio::test]
async fn jwt_verified_via_discovery_and_jwks() {
    let server = issuer().await;
    let v = verifier(&server, false);
    let id = v.verify(&sign(&server.uri(), json!({}))).await.unwrap();
    assert_eq!(id.tenant_id.as_str(), "org-1");
    assert_eq!(id.user_id, "user-1");
    assert_eq!(id.role, UserRole::SuperAdmin);
}

#[tokio::test]
async fn jwt_with_wrong_audience_issuer_or_expiry_is_rejected() {
    let server = issuer().await;
    let v = verifier(&server, false);
    for bad in [
        sign(&server.uri(), json!({ "aud": ["other-project"] })),
        sign("https://evil.example", json!({})),
        sign(&server.uri(), json!({ "exp": now() - 3600 })),
    ] {
        assert!(matches!(
            v.verify(&bad).await,
            Err(AuthError::InvalidToken(_))
        ));
    }
}

#[tokio::test]
async fn jwt_without_org_claim_is_rejected() {
    let server = issuer().await;
    let v = verifier(&server, false);
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("k1".into());
    let token = encode(
        &header,
        &json!({ "iss": server.uri(), "sub": "u", "aud": [AUD], "exp": now() + 600 }),
        &EncodingKey::from_rsa_pem(key().pem.as_bytes()).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        v.verify(&token).await,
        Err(AuthError::InvalidToken(_))
    ));
}

#[tokio::test]
async fn jwt_signed_by_another_key_is_rejected() {
    let server = issuer().await;
    let v = verifier(&server, false);
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("unknown-kid".into());
    let token = encode(
        &header,
        &json!({ "iss": server.uri(), "sub": "u", "aud": [AUD], "exp": now() + 600, ORG_CLAIM: "o" }),
        &EncodingKey::from_rsa_pem(key().pem.as_bytes()).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        v.verify(&token).await,
        Err(AuthError::InvalidToken(_))
    ));
}

fn introspection(server_uri: &str, active: bool, aud: &str) -> serde_json::Value {
    json!({
        "active": active, "sub": "user-2", "iss": server_uri, "aud": [aud],
        "exp": now() + 600, ORG_CLAIM: "org-2",
        "urn:conusai:plan_tier": "enterprise"
    })
}

#[tokio::test]
async fn opaque_token_introspected_once_then_cached() {
    let server = issuer().await;
    Mock::given(method("POST"))
        .and(path("/oauth/v2/introspect"))
        .and(body_string_contains("token=opaque-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(introspection(
            &server.uri(),
            true,
            AUD,
        )))
        .expect(1)
        .mount(&server)
        .await;
    let v = verifier(&server, true);
    let a = v.verify("opaque-1").await.unwrap();
    let b = v.verify("opaque-1").await.unwrap();
    assert_eq!(a, b);
    assert_eq!(a.tenant_id.as_str(), "org-2");
    assert_eq!(a.role, UserRole::User);
}

#[tokio::test]
async fn inactive_or_foreign_opaque_tokens_are_rejected() {
    let server = issuer().await;
    Mock::given(method("POST"))
        .and(path("/oauth/v2/introspect"))
        .and(body_string_contains("token=inactive"))
        .respond_with(ResponseTemplate::new(200).set_body_json(introspection(
            &server.uri(),
            false,
            AUD,
        )))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/oauth/v2/introspect"))
        .and(body_string_contains("token=foreign"))
        .respond_with(ResponseTemplate::new(200).set_body_json(introspection(
            &server.uri(),
            true,
            "another-app",
        )))
        .mount(&server)
        .await;
    let v = verifier(&server, true);
    assert!(matches!(
        v.verify("inactive").await,
        Err(AuthError::InvalidToken(_))
    ));
    assert!(matches!(
        v.verify("foreign").await,
        Err(AuthError::InvalidToken(_))
    ));
}

#[tokio::test]
async fn introspection_outage_is_unavailable() {
    let server = issuer().await;
    Mock::given(method("POST"))
        .and(path("/oauth/v2/introspect"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let v = verifier(&server, true);
    assert!(matches!(
        v.verify("whatever").await,
        Err(AuthError::Unavailable(_))
    ));
}

#[tokio::test]
async fn opaque_tokens_refused_without_introspection() {
    let server = issuer().await;
    let v = verifier(&server, false);
    assert!(matches!(
        v.verify("opaque").await,
        Err(AuthError::InvalidToken(_))
    ));
}
