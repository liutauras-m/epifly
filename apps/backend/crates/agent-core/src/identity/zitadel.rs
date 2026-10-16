//! Zitadel access-token verification.
//!
//! - **JWT access tokens** are verified locally: signature against the issuer's
//!   JWKS (from OIDC discovery), `iss` = `ZITADEL_ISSUER`, `aud` contains
//!   `ZITADEL_AUDIENCE`, `exp`/`nbf` with 60 s leeway. Only RS256 is accepted.
//! - **Opaque access tokens** are checked with the introspection endpoint, when
//!   `ZITADEL_INTROSPECTION_CLIENT_ID`/`_SECRET` are configured; otherwise they
//!   are refused. A JWT-shaped token never falls back to introspection.
//!
//! The tenant is the user's organization (`urn:zitadel:iam:user:resourceowner:id`,
//! present when the client requests the `urn:zitadel:iam:user:resourceowner`
//! scope); a token without it is rejected. Roles come from
//! `urn:zitadel:iam:org:project:roles`. Nothing about plans is read from tokens.

use super::{AuthError, Identity, TokenVerifier, role_from_project_roles};
use async_trait::async_trait;
use common::types::TenantId;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use moka::future::Cache;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::debug;

pub const ORG_CLAIM: &str = "urn:zitadel:iam:user:resourceowner:id";
pub const ROLES_CLAIM: &str = "urn:zitadel:iam:org:project:roles";

/// JWKS keys are refetched after this long.
const JWKS_TTL: Duration = Duration::from_secs(600);
/// An unknown `kid` triggers at most one refetch per this interval (no kid-flood DoS).
const JWKS_MIN_REFRESH: Duration = Duration::from_secs(30);
/// Introspection results are cached this long at most.
const INTROSPECTION_CACHE_TTL: Duration = Duration::from_secs(60);
const LEEWAY_SECS: u64 = 60;

/// Credentials for the introspection endpoint (a Zitadel API application with Basic auth).
#[derive(Clone)]
pub struct IntrospectionClient {
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Clone)]
pub struct ZitadelConfig {
    /// Issuer URL exactly as it appears in `iss` (no trailing slash).
    pub issuer: String,
    /// Required `aud` value (the Zitadel project or API app id).
    pub audience: String,
    pub introspection: Option<IntrospectionClient>,
}

impl ZitadelConfig {
    /// Validate the URL shape: https, except loopback hosts (local test servers).
    pub fn validate(&self) -> Result<(), String> {
        require_secure_url(&self.issuer)?;
        if self.audience.trim().is_empty() {
            return Err("audience is empty".into());
        }
        Ok(())
    }
}

fn require_secure_url(raw: &str) -> Result<(), String> {
    let url = url::Url::parse(raw).map_err(|e| format!("{raw}: {e}"))?;
    match url.scheme() {
        "https" => Ok(()),
        "http" if is_loopback(&url) => Ok(()),
        other => Err(format!(
            "{raw}: scheme {other} is not allowed (https required)"
        )),
    }
}

fn is_loopback(url: &url::Url) -> bool {
    match url.host() {
        Some(url::Host::Domain(d)) => d == "localhost",
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    }
}

#[derive(Debug, Deserialize)]
struct OidcDiscovery {
    issuer: String,
    jwks_uri: String,
}

#[derive(Debug, Deserialize)]
struct Jwk {
    kty: String,
    #[serde(default)]
    kid: Option<String>,
    #[serde(default)]
    n: Option<String>,
    #[serde(default)]
    e: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JwkSet {
    keys: Vec<Jwk>,
}

#[derive(Default)]
struct JwkState {
    keys: HashMap<String, DecodingKey>,
    fetched_at: Option<Instant>,
}

impl JwkState {
    fn is_stale(&self) -> bool {
        self.fetched_at.is_none_or(|t| t.elapsed() > JWKS_TTL)
    }
    fn may_refresh_for_unknown_kid(&self) -> bool {
        self.fetched_at
            .is_none_or(|t| t.elapsed() > JWKS_MIN_REFRESH)
    }
}

/// Claims we read from a verified JWT access token.
#[derive(Debug, Deserialize)]
struct AccessTokenClaims {
    sub: String,
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct IntrospectionResponse {
    active: bool,
    #[serde(default)]
    sub: Option<String>,
    #[serde(default)]
    iss: Option<String>,
    #[serde(default)]
    aud: Option<serde_json::Value>,
    #[serde(default)]
    exp: Option<u64>,
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

pub struct ZitadelVerifier {
    config: ZitadelConfig,
    http: reqwest::Client,
    jwks_uri: tokio::sync::OnceCell<String>,
    jwks: tokio::sync::RwLock<JwkState>,
    jwks_refresh: tokio::sync::Mutex<()>,
    introspection_cache: Cache<String, Identity>,
}

impl ZitadelVerifier {
    pub fn new(config: ZitadelConfig) -> Result<Self, String> {
        config.validate()?;
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            config,
            http,
            jwks_uri: tokio::sync::OnceCell::new(),
            jwks: tokio::sync::RwLock::new(JwkState::default()),
            jwks_refresh: tokio::sync::Mutex::new(()),
            introspection_cache: Cache::builder()
                .max_capacity(10_000)
                .time_to_live(INTROSPECTION_CACHE_TTL)
                .build(),
        })
    }

    async fn jwks_uri(&self) -> Result<&str, AuthError> {
        self.jwks_uri
            .get_or_try_init(|| async {
                let url = format!("{}/.well-known/openid-configuration", self.config.issuer);
                let resp = self
                    .http
                    .get(&url)
                    .send()
                    .await
                    .map_err(|e| AuthError::Unavailable(format!("discovery: {e}")))?;
                if !resp.status().is_success() {
                    return Err(AuthError::Unavailable(format!(
                        "discovery HTTP {}",
                        resp.status()
                    )));
                }
                let d: OidcDiscovery = resp
                    .json()
                    .await
                    .map_err(|e| AuthError::Unavailable(format!("discovery parse: {e}")))?;
                if d.issuer.trim_end_matches('/') != self.config.issuer {
                    return Err(AuthError::Unavailable(format!(
                        "discovery issuer {} does not match ZITADEL_ISSUER",
                        d.issuer
                    )));
                }
                require_secure_url(&d.jwks_uri).map_err(AuthError::Unavailable)?;
                Ok(d.jwks_uri)
            })
            .await
            .map(String::as_str)
    }

    async fn refresh_jwks(&self, for_unknown_kid: bool) -> Result<(), AuthError> {
        let _single_flight = self.jwks_refresh.lock().await;
        {
            let state = self.jwks.read().await;
            let needed = if for_unknown_kid {
                state.may_refresh_for_unknown_kid()
            } else {
                state.is_stale()
            };
            if !needed {
                return Ok(());
            }
        }
        let uri = self.jwks_uri().await?.to_string();
        let resp = self
            .http
            .get(&uri)
            .send()
            .await
            .map_err(|e| AuthError::Unavailable(format!("JWKS: {e}")))?;
        if !resp.status().is_success() {
            return Err(AuthError::Unavailable(format!(
                "JWKS HTTP {}",
                resp.status()
            )));
        }
        let set: JwkSet = resp
            .json()
            .await
            .map_err(|e| AuthError::Unavailable(format!("JWKS parse: {e}")))?;

        let mut keys = HashMap::new();
        for jwk in set.keys {
            if jwk.kty != "RSA" {
                continue;
            }
            if let (Some(kid), Some(n), Some(e)) = (jwk.kid, jwk.n, jwk.e)
                && let Ok(key) = DecodingKey::from_rsa_components(&n, &e)
            {
                keys.insert(kid, key);
            }
        }
        let mut state = self.jwks.write().await;
        state.keys = keys;
        state.fetched_at = Some(Instant::now());
        debug!(keys = state.keys.len(), "JWKS refreshed");
        Ok(())
    }

    async fn key_for(&self, kid: &str) -> Result<DecodingKey, AuthError> {
        {
            let state = self.jwks.read().await;
            if !state.is_stale()
                && let Some(k) = state.keys.get(kid)
            {
                return Ok(k.clone());
            }
        }
        // Stale keyset: regular refresh. Fresh keyset without the kid: rate-limited refresh.
        let stale = self.jwks.read().await.is_stale();
        self.refresh_jwks(!stale).await?;
        self.jwks
            .read()
            .await
            .keys
            .get(kid)
            .cloned()
            .ok_or_else(|| AuthError::InvalidToken(format!("unknown signing key {kid}")))
    }

    async fn verify_jwt(&self, token: &str) -> Result<Identity, AuthError> {
        let header =
            decode_header(token).map_err(|e| AuthError::InvalidToken(format!("header: {e}")))?;
        if header.alg != Algorithm::RS256 {
            return Err(AuthError::InvalidToken(format!(
                "algorithm {:?} not accepted",
                header.alg
            )));
        }
        let kid = header
            .kid
            .ok_or_else(|| AuthError::InvalidToken("token has no kid".into()))?;
        let key = self.key_for(&kid).await?;
        validate_jwt(token, &key, &self.config)
    }

    async fn verify_opaque(&self, token: &str) -> Result<Identity, AuthError> {
        let Some(client) = &self.config.introspection else {
            return Err(AuthError::InvalidToken(
                "opaque tokens are not accepted (introspection not configured)".into(),
            ));
        };
        let cache_key = blake3::hash(token.as_bytes()).to_hex().to_string();
        if let Some(hit) = self.introspection_cache.get(&cache_key).await {
            return Ok(hit);
        }
        let url = format!("{}/oauth/v2/introspect", self.config.issuer);
        let resp = self
            .http
            .post(&url)
            .basic_auth(&client.client_id, Some(&client.client_secret))
            .form(&[("token", token)])
            .send()
            .await
            .map_err(|e| AuthError::Unavailable(format!("introspection: {e}")))?;
        if !resp.status().is_success() {
            return Err(AuthError::Unavailable(format!(
                "introspection HTTP {}",
                resp.status()
            )));
        }
        let intro: IntrospectionResponse = resp
            .json()
            .await
            .map_err(|e| AuthError::Unavailable(format!("introspection parse: {e}")))?;
        let identity = identity_from_introspection(intro, &self.config, unix_now())?;
        self.introspection_cache
            .insert(cache_key, identity.clone())
            .await;
        Ok(identity)
    }
}

#[async_trait]
impl TokenVerifier for ZitadelVerifier {
    async fn verify(&self, token: &str) -> Result<Identity, AuthError> {
        if looks_like_jwt(token) {
            self.verify_jwt(token).await
        } else {
            self.verify_opaque(token).await
        }
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Three base64url segments whose first one decodes as a JWT header.
fn looks_like_jwt(token: &str) -> bool {
    token.split('.').count() == 3 && decode_header(token).is_ok()
}

/// Decode and validate a JWT with an already-resolved key (pure; unit-tested).
pub(crate) fn validate_jwt(
    token: &str,
    key: &DecodingKey,
    config: &ZitadelConfig,
) -> Result<Identity, AuthError> {
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[&config.issuer]);
    validation.set_audience(&[&config.audience]);
    validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
    validation.leeway = LEEWAY_SECS;
    validation.validate_nbf = true;

    let claims = decode::<AccessTokenClaims>(token, key, &validation)
        .map_err(|e| AuthError::InvalidToken(e.to_string()))?
        .claims;
    identity_from_claims(claims.sub, &claims.extra)
}

fn identity_from_claims(
    sub: String,
    extra: &serde_json::Map<String, serde_json::Value>,
) -> Result<Identity, AuthError> {
    if sub.trim().is_empty() {
        return Err(AuthError::InvalidToken("empty sub".into()));
    }
    let tenant_id = extra
        .get(ORG_CLAIM)
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| AuthError::InvalidToken(format!("missing {ORG_CLAIM} claim")))?;
    let role = role_from_project_roles(
        extra
            .get(ROLES_CLAIM)
            .and_then(|v| v.as_object())
            .into_iter()
            .flat_map(|m| m.keys().map(String::as_str)),
    );
    let email_verified = extra
        .get("email_verified")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let email = extra
        .get("email")
        .and_then(|v| v.as_str())
        .filter(|_| email_verified)
        .map(String::from);
    Ok(Identity {
        user_id: sub,
        tenant_id: TenantId::from(tenant_id.to_string()),
        email,
        role,
    })
}

fn identity_from_introspection(
    intro: IntrospectionResponse,
    config: &ZitadelConfig,
    now: u64,
) -> Result<Identity, AuthError> {
    if !intro.active {
        return Err(AuthError::InvalidToken("token is not active".into()));
    }
    match intro.iss.as_deref().map(|i| i.trim_end_matches('/')) {
        Some(iss) if iss == config.issuer => {}
        _ => return Err(AuthError::InvalidToken("issuer mismatch".into())),
    }
    let aud_ok = match &intro.aud {
        Some(serde_json::Value::String(a)) => a == &config.audience,
        Some(serde_json::Value::Array(list)) => list
            .iter()
            .any(|a| a.as_str() == Some(config.audience.as_str())),
        _ => false,
    };
    if !aud_ok {
        return Err(AuthError::InvalidToken("audience mismatch".into()));
    }
    match intro.exp {
        Some(exp) if exp + LEEWAY_SECS >= now => {}
        _ => return Err(AuthError::InvalidToken("token expired".into())),
    }
    let sub = intro
        .sub
        .ok_or_else(|| AuthError::InvalidToken("no sub".into()))?;
    identity_from_claims(sub, &intro.extra)
}

#[cfg(test)]
pub(crate) mod test_keys {
    //! An RSA key generated once per test process — no key material is committed.
    use base64::Engine as _;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use rsa::pkcs1::EncodeRsaPrivateKey;
    use rsa::traits::PublicKeyParts;
    use std::sync::OnceLock;

    pub struct TestKey {
        pub private_pem: String,
        pub n: String,
        pub e: String,
    }

    pub fn key() -> &'static TestKey {
        static KEY: OnceLock<TestKey> = OnceLock::new();
        KEY.get_or_init(|| {
            let mut rng = rand::thread_rng();
            let private = rsa::RsaPrivateKey::new(&mut rng, 2048).expect("generate RSA key");
            TestKey {
                private_pem: private
                    .to_pkcs1_pem(rsa::pkcs8::LineEnding::LF)
                    .expect("encode PEM")
                    .to_string(),
                n: URL_SAFE_NO_PAD.encode(private.n().to_bytes_be()),
                e: URL_SAFE_NO_PAD.encode(private.e().to_bytes_be()),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::tenant::UserRole;
    use jsonwebtoken::{EncodingKey, Header, encode};
    use serde_json::json;

    const ISS: &str = "https://auth.test.example";
    const AUD: &str = "gateway-project";

    fn config() -> ZitadelConfig {
        ZitadelConfig {
            issuer: ISS.into(),
            audience: AUD.into(),
            introspection: None,
        }
    }

    fn sign(claims: serde_json::Value) -> String {
        let k = test_keys::key();
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some("kid-1".into());
        encode(
            &header,
            &claims,
            &EncodingKey::from_rsa_pem(k.private_pem.as_bytes()).unwrap(),
        )
        .unwrap()
    }

    fn decoding_key() -> DecodingKey {
        let k = test_keys::key();
        DecodingKey::from_rsa_components(&k.n, &k.e).unwrap()
    }

    fn claims(overrides: serde_json::Value) -> serde_json::Value {
        let now = unix_now();
        let mut base = json!({
            "iss": ISS,
            "sub": "user-1",
            "aud": [AUD, "other-client"],
            "exp": now + 3600,
            "iat": now,
            "nbf": now - 5,
            ORG_CLAIM: "org-42",
            ROLES_CLAIM: { "admin": { "org-42": "acme" } },
            "email": "a@example.com",
            "email_verified": true
        });
        for (k, v) in overrides.as_object().unwrap() {
            if v.is_null() {
                base.as_object_mut().unwrap().remove(k);
            } else {
                base[k] = v.clone();
            }
        }
        base
    }

    #[test]
    fn accepts_valid_token() {
        let id = validate_jwt(&sign(claims(json!({}))), &decoding_key(), &config()).unwrap();
        assert_eq!(id.user_id, "user-1");
        assert_eq!(id.tenant_id.as_str(), "org-42");
        assert_eq!(id.role, UserRole::Admin);
        assert_eq!(id.email.as_deref(), Some("a@example.com"));
    }

    #[test]
    fn super_admin_role() {
        let t = sign(claims(json!({ ROLES_CLAIM: { "super_admin": {} } })));
        let id = validate_jwt(&t, &decoding_key(), &config()).unwrap();
        assert_eq!(id.role, UserRole::SuperAdmin);
    }

    #[test]
    fn rejects_wrong_issuer() {
        let t = sign(claims(json!({ "iss": "https://evil.example" })));
        assert!(validate_jwt(&t, &decoding_key(), &config()).is_err());
    }

    #[test]
    fn rejects_wrong_audience() {
        let t = sign(claims(json!({ "aud": ["someone-else"] })));
        assert!(validate_jwt(&t, &decoding_key(), &config()).is_err());
    }

    #[test]
    fn rejects_expired() {
        let t = sign(claims(json!({ "exp": unix_now() - 3600 })));
        assert!(validate_jwt(&t, &decoding_key(), &config()).is_err());
    }

    #[test]
    fn rejects_missing_org() {
        let t = sign(claims(json!({ ORG_CLAIM: null })));
        let err = validate_jwt(&t, &decoding_key(), &config()).unwrap_err();
        assert!(err.to_string().contains(ORG_CLAIM));
    }

    #[test]
    fn ignores_unverified_email_and_plan_claims() {
        let t = sign(claims(json!({
            "email_verified": false,
            "urn:conusai:plan_tier": "enterprise"
        })));
        let id = validate_jwt(&t, &decoding_key(), &config()).unwrap();
        assert_eq!(id.email, None);
    }

    #[test]
    fn rejects_hs256() {
        let mut header = Header::new(Algorithm::HS256);
        header.kid = Some("kid-1".into());
        let t = encode(&header, &claims(json!({})), &EncodingKey::from_secret(b"x")).unwrap();
        assert!(validate_jwt(&t, &decoding_key(), &config()).is_err());
    }

    #[test]
    fn introspection_checks_issuer_audience_and_expiry() {
        let now = unix_now();
        let ok = |v: serde_json::Value| {
            identity_from_introspection(serde_json::from_value(v).unwrap(), &config(), now)
        };
        let base = json!({
            "active": true, "sub": "u", "iss": ISS, "aud": [AUD], "exp": now + 60,
            ORG_CLAIM: "org-1"
        });
        assert_eq!(ok(base.clone()).unwrap().tenant_id.as_str(), "org-1");

        let mut inactive = base.clone();
        inactive["active"] = json!(false);
        assert!(ok(inactive).is_err());

        let mut wrong_aud = base.clone();
        wrong_aud["aud"] = json!("nope");
        assert!(ok(wrong_aud).is_err());

        let mut wrong_iss = base.clone();
        wrong_iss["iss"] = json!("https://evil.example");
        assert!(ok(wrong_iss).is_err());

        let mut expired = base.clone();
        expired["exp"] = json!(now - 3600);
        assert!(ok(expired).is_err());

        let mut no_org = base;
        no_org.as_object_mut().unwrap().remove(ORG_CLAIM);
        assert!(ok(no_org).is_err());
    }

    #[test]
    fn config_requires_https_except_loopback() {
        let mut c = config();
        assert!(c.validate().is_ok());
        c.issuer = "http://auth.example".into();
        assert!(c.validate().is_err());
        c.issuer = "http://127.0.0.1:8080".into();
        assert!(c.validate().is_ok());
        c.issuer = "http://localhost:8080".into();
        assert!(c.validate().is_ok());
    }

    #[test]
    fn jwt_shape_detection() {
        assert!(looks_like_jwt(&sign(claims(json!({})))));
        assert!(!looks_like_jwt("opaque-token-value"));
        assert!(!looks_like_jwt("a.b.c"));
    }

    #[tokio::test]
    async fn opaque_token_refused_without_introspection() {
        let v = ZitadelVerifier::new(config()).unwrap();
        let err = v.verify("opaque-token-value").await.unwrap_err();
        assert!(matches!(err, AuthError::InvalidToken(_)));
    }
}
