//! OIDC authorization code + PKCE for a native app (RFC 8252): loopback
//! redirect on 127.0.0.1, `state` and `nonce`, token exchange and refresh.
//!
//! The ID token is received directly from the token endpoint over TLS, so its
//! claims are validated (iss, aud, azp, exp, nonce) without a signature check
//! (OIDC Core 3.1.3.7, step 6). Release builds require an https issuer.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

pub const SCOPE: &str = "openid profile email offline_access";
const CLOCK_SKEW_S: u64 = 60;

#[derive(Debug, Clone, Deserialize)]
pub struct Discovery {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    #[serde(default)]
    pub end_session_endpoint: Option<String>,
    #[serde(default)]
    pub revocation_endpoint: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub id_token: Option<String>,
    #[serde(default)]
    pub expires_in: Option<u64>,
}

/// Display identity of the signed-in user (no secrets).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    pub sub: String,
    pub name: String,
    pub email: Option<String>,
}

/// Outcome of a refresh-token grant.
pub enum Refresh {
    Tokens(TokenResponse),
    /// `invalid_grant`: the refresh token is revoked or expired — sign in again.
    Dead,
}

/// A pending authorization request: open `url`, then wait for the callback.
pub struct AuthorizationRequest {
    pub url: String,
    pub state: String,
    pub nonce: String,
    pub verifier: String,
    pub redirect_uri: String,
}

pub fn random_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn code_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn same_issuer(a: &str, b: &str) -> bool {
    a.trim_end_matches('/') == b.trim_end_matches('/')
}

/// Constant-time string comparison for `state` and `nonce`.
pub fn ct_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Release builds only talk to an https issuer; debug builds also accept a
/// loopback http issuer (local mock IdP).
pub fn check_issuer_url(issuer: &str) -> Result<(), String> {
    let url = Url::parse(issuer).map_err(|e| format!("invalid issuer URL: {e}"))?;
    let loopback = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    if url.scheme() == "https" || (cfg!(debug_assertions) && url.scheme() == "http" && loopback) {
        Ok(())
    } else {
        Err("the issuer must use https".into())
    }
}

pub async fn discover(http: &reqwest::Client, issuer: &str) -> Result<Discovery, String> {
    check_issuer_url(issuer)?;
    let url = format!("{}/.well-known/openid-configuration", issuer.trim_end_matches('/'));
    let discovery: Discovery = http
        .get(url)
        .send()
        .await
        .map_err(|e| format!("discovery request failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("discovery failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("discovery document unreadable: {e}"))?;
    if !same_issuer(&discovery.issuer, issuer) {
        return Err("discovery issuer does not match the configured issuer".into());
    }
    Ok(discovery)
}

pub fn authorization_request(
    discovery: &Discovery,
    client_id: &str,
    port: u16,
) -> Result<AuthorizationRequest, String> {
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");
    let state = random_token();
    let nonce = random_token();
    let verifier = random_token();
    let mut url = Url::parse(&discovery.authorization_endpoint)
        .map_err(|e| format!("invalid authorization endpoint: {e}"))?;
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("scope", SCOPE)
        .append_pair("state", &state)
        .append_pair("nonce", &nonce)
        .append_pair("code_challenge", &code_challenge(&verifier))
        .append_pair("code_challenge_method", "S256");
    Ok(AuthorizationRequest { url: url.into(), state, nonce, verifier, redirect_uri })
}

async fn respond(stream: &mut tokio::net::TcpStream, status: &str, message: &str) {
    let body = format!(
        "<!doctype html><meta charset=utf-8><title>ConusAI</title><p style=\"font-family:system-ui\">{message}</p>"
    );
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
}

/// Accept loopback connections until the IdP redirects back with our `state`.
/// Requests with a missing or wrong `state` are answered and ignored, so a
/// local process cannot inject a code or end the wait early. Returns the code.
pub async fn await_callback(
    listener: TcpListener,
    expected_state: &str,
    timeout: Duration,
) -> Result<String, String> {
    let wait = async {
        loop {
            let (mut stream, _) = listener
                .accept()
                .await
                .map_err(|e| format!("callback listener failed: {e}"))?;
            let mut buf = vec![0u8; 16 * 1024];
            let n = match tokio::time::timeout(Duration::from_secs(5), stream.read(&mut buf)).await {
                Ok(Ok(n)) => n,
                _ => continue,
            };
            let head = String::from_utf8_lossy(&buf[..n]);
            let target = head.lines().next().and_then(|line| {
                let mut parts = line.split_whitespace();
                match (parts.next(), parts.next()) {
                    (Some("GET"), Some(target)) => Some(target.to_owned()),
                    _ => None,
                }
            });
            let Some(url) = target.and_then(|t| Url::parse(&format!("http://127.0.0.1{t}")).ok()) else {
                respond(&mut stream, "400 Bad Request", "Bad request.").await;
                continue;
            };
            if url.path() != "/callback" {
                respond(&mut stream, "404 Not Found", "Not found.").await;
                continue;
            }
            let params: HashMap<String, String> = url.query_pairs().into_owned().collect();
            if !params.get("state").is_some_and(|s| ct_eq(s, expected_state)) {
                respond(&mut stream, "400 Bad Request", "This sign-in link is not valid for this app.").await;
                continue;
            }
            if let Some(error) = params.get("error") {
                respond(&mut stream, "200 OK", "Sign-in was not completed. You can close this tab.").await;
                let detail = params.get("error_description").unwrap_or(error);
                return Err(format!("sign-in failed: {detail}"));
            }
            let Some(code) = params.get("code").cloned() else {
                respond(&mut stream, "400 Bad Request", "No authorization code.").await;
                return Err("the identity provider returned no code".into());
            };
            respond(&mut stream, "200 OK", "Signed in. You can close this tab and return to ConusAI.").await;
            return Ok(code);
        }
    };
    tokio::time::timeout(timeout, wait)
        .await
        .map_err(|_| "sign-in timed out".to_string())?
}

#[derive(Deserialize)]
struct TokenError {
    error: String,
}

async fn token_request(
    http: &reqwest::Client,
    endpoint: &str,
    form: &[(&str, &str)],
) -> Result<Result<TokenResponse, String>, String> {
    let res = http
        .post(endpoint)
        .form(form)
        .send()
        .await
        .map_err(|e| format!("token request failed: {e}"))?;
    if res.status().is_success() {
        let tokens = res
            .json::<TokenResponse>()
            .await
            .map_err(|e| format!("token response unreadable: {e}"))?;
        return Ok(Ok(tokens));
    }
    let status = res.status();
    let error = res
        .json::<TokenError>()
        .await
        .map(|e| e.error)
        .unwrap_or_else(|_| status.to_string());
    Ok(Err(error))
}

pub async fn exchange_code(
    http: &reqwest::Client,
    discovery: &Discovery,
    client_id: &str,
    request: &AuthorizationRequest,
    code: &str,
) -> Result<TokenResponse, String> {
    token_request(
        http,
        &discovery.token_endpoint,
        &[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", &request.redirect_uri),
            ("client_id", client_id),
            ("code_verifier", &request.verifier),
        ],
    )
    .await?
    .map_err(|e| format!("code exchange refused: {e}"))
}

pub async fn refresh(
    http: &reqwest::Client,
    discovery: &Discovery,
    client_id: &str,
    refresh_token: &str,
) -> Result<Refresh, String> {
    match token_request(
        http,
        &discovery.token_endpoint,
        &[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", client_id),
        ],
    )
    .await?
    {
        Ok(tokens) => Ok(Refresh::Tokens(tokens)),
        Err(e) if e == "invalid_grant" => Ok(Refresh::Dead),
        Err(e) => Err(format!("refresh refused: {e}")),
    }
}

pub async fn revoke(http: &reqwest::Client, discovery: &Discovery, client_id: &str, refresh_token: &str) {
    if let Some(endpoint) = &discovery.revocation_endpoint {
        let _ = http
            .post(endpoint)
            .form(&[
                ("token", refresh_token),
                ("token_type_hint", "refresh_token"),
                ("client_id", client_id),
            ])
            .send()
            .await;
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Audience {
    One(String),
    Many(Vec<String>),
}

#[derive(Deserialize)]
struct IdClaims {
    iss: String,
    sub: String,
    aud: Audience,
    exp: u64,
    #[serde(default)]
    azp: Option<String>,
    #[serde(default)]
    nonce: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    preferred_username: Option<String>,
    #[serde(default)]
    email: Option<String>,
}

/// Validate the ID token's claims and return the user's identity.
pub fn validate_id_token(
    id_token: &str,
    issuer: &str,
    client_id: &str,
    expected_nonce: &str,
    now_s: u64,
) -> Result<Identity, String> {
    let payload = id_token.split('.').nth(1).ok_or("malformed ID token")?;
    let bytes = URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .map_err(|_| "malformed ID token payload")?;
    let claims: IdClaims = serde_json::from_slice(&bytes).map_err(|_| "unreadable ID token claims")?;

    if !same_issuer(&claims.iss, issuer) {
        return Err("ID token issuer mismatch".into());
    }
    let audience_ok = match &claims.aud {
        Audience::One(a) => a == client_id,
        Audience::Many(list) => list.iter().any(|a| a == client_id),
    };
    if !audience_ok {
        return Err("ID token audience mismatch".into());
    }
    if claims.azp.as_deref().is_some_and(|azp| azp != client_id) {
        return Err("ID token authorized party mismatch".into());
    }
    if claims.exp + CLOCK_SKEW_S <= now_s {
        return Err("ID token expired".into());
    }
    if !claims.nonce.as_deref().is_some_and(|n| ct_eq(n, expected_nonce)) {
        return Err("ID token nonce mismatch".into());
    }
    let name = claims
        .name
        .or(claims.preferred_username)
        .or_else(|| claims.email.clone())
        .unwrap_or_else(|| claims.sub.clone());
    Ok(Identity { sub: claims.sub, name, email: claims.email })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id_token(claims: serde_json::Value) -> String {
        format!(
            "{}.{}.sig",
            URL_SAFE_NO_PAD.encode(br#"{"alg":"RS256"}"#),
            URL_SAFE_NO_PAD.encode(claims.to_string())
        )
    }

    fn claims() -> serde_json::Value {
        serde_json::json!({
            "iss": "https://auth.example.test",
            "sub": "user-1",
            "aud": ["project-1", "native-client"],
            "azp": "native-client",
            "exp": 2_000_000_000u64,
            "nonce": "n-123",
            "name": "Ada",
            "email": "ada@example.test"
        })
    }

    const NOW: u64 = 1_900_000_000;

    #[test]
    fn accepts_valid_claims() {
        let id = validate_id_token(&id_token(claims()), "https://auth.example.test/", "native-client", "n-123", NOW)
            .expect("valid");
        assert_eq!(id, Identity { sub: "user-1".into(), name: "Ada".into(), email: Some("ada@example.test".into()) });
    }

    #[test]
    fn rejects_wrong_nonce_audience_issuer_azp_and_expiry() {
        let token = id_token(claims());
        assert!(validate_id_token(&token, "https://auth.example.test", "native-client", "other", NOW).is_err());
        assert!(validate_id_token(&token, "https://auth.example.test", "someone-else", "n-123", NOW).is_err());
        assert!(validate_id_token(&token, "https://evil.example", "native-client", "n-123", NOW).is_err());
        assert!(validate_id_token(&token, "https://auth.example.test", "native-client", "n-123", 2_100_000_000).is_err());
        let mut c = claims();
        c["azp"] = "web-client".into();
        assert!(validate_id_token(&id_token(c), "https://auth.example.test", "native-client", "n-123", NOW).is_err());
        let mut c = claims();
        c.as_object_mut().unwrap().remove("nonce");
        assert!(validate_id_token(&id_token(c), "https://auth.example.test", "native-client", "n-123", NOW).is_err());
    }

    #[test]
    fn pkce_challenge_matches_rfc7636_example() {
        assert_eq!(
            code_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn authorization_request_carries_pkce_state_and_nonce() {
        let d = Discovery {
            issuer: "https://auth.example.test".into(),
            authorization_endpoint: "https://auth.example.test/oauth/v2/authorize".into(),
            token_endpoint: "https://auth.example.test/oauth/v2/token".into(),
            end_session_endpoint: None,
            revocation_endpoint: None,
        };
        let req = authorization_request(&d, "native-client", 49152).unwrap();
        let url = Url::parse(&req.url).unwrap();
        let q: HashMap<String, String> = url.query_pairs().into_owned().collect();
        assert_eq!(q["state"], req.state);
        assert_eq!(q["nonce"], req.nonce);
        assert_eq!(q["code_challenge"], code_challenge(&req.verifier));
        assert_eq!(q["code_challenge_method"], "S256");
        assert_eq!(q["redirect_uri"], "http://127.0.0.1:49152/callback");
        assert_ne!(req.state, req.nonce);
    }

    #[test]
    fn issuer_must_be_https_in_release() {
        assert!(check_issuer_url("https://auth.example.test").is_ok());
        assert!(check_issuer_url("http://auth.example.test").is_err());
    }

    async fn get(port: u16, path: &str) -> String {
        let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        s.write_all(format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").as_bytes()).await.unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).await.unwrap();
        out
    }

    #[tokio::test]
    async fn callback_ignores_wrong_state_and_returns_code_for_ours() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let waiter = tokio::spawn(async move { await_callback(listener, "good-state", Duration::from_secs(5)).await });
        assert!(get(port, "/favicon.ico").await.starts_with("HTTP/1.1 404"));
        assert!(get(port, "/callback?code=evil&state=bad").await.starts_with("HTTP/1.1 400"));
        assert!(get(port, "/callback?code=abc&state=good-state").await.starts_with("HTTP/1.1 200"));
        assert_eq!(waiter.await.unwrap().unwrap(), "abc");
    }
}
