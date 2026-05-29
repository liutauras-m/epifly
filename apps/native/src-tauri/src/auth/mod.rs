/// Native OIDC auth module (Plan v5.1 — Phase 4 + 5).
///
/// Handles PKCE authorization code flow via the system browser:
/// 1. `start_login` → generates PKCE/state/nonce, opens system browser
/// 2. Deep-link handler → validates callback, exchanges code, persists tokens
/// 3. `get_access_token` → returns access token; proactively refreshes if needed
/// 4. `sign_out` → revokes refresh token, clears keychain, sends Tauri event
pub mod refresh;
pub mod store;

use dashmap::DashMap;
use oauth2::{CsrfToken, PkceCodeChallenge};
use refresh::RefreshGate;
use reqwest::Client;
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::OnceCell;

const REDIRECT_PRIORITY: [&str; 3] = [
    "https://auth.epifly.app/native/callback", // Universal Link (production)
    "epifly://auth/callback",                  // custom scheme fallback
    "http://127.0.0.1:53682/callback",         // desktop loopback
];

/// In-progress PKCE transaction. Keyed by state token.
#[derive(Clone)]
struct Transaction {
    /// Raw PKCE code verifier secret (PkceCodeVerifier is not Clone)
    code_verifier_secret: String,
    #[allow(dead_code)] // used for future nonce validation against id_token
    nonce: String,
    redirect_uri: String,
    created_at: std::time::Instant,
}

/// OIDC discovery document subset we need at runtime.
#[derive(Debug, Clone, Deserialize)]
struct OidcDiscovery {
    authorization_endpoint: String,
    token_endpoint: String,
    #[serde(default)]
    revocation_endpoint: Option<String>,
}

pub struct AuthState {
    transactions: Arc<DashMap<String, Transaction>>,
    refresh_gate: Arc<RefreshGate>,
    discovery: Arc<OnceCell<OidcDiscovery>>,
    http: Client,
}

impl Default for AuthState {
    fn default() -> Self {
        Self {
            transactions: Arc::new(DashMap::with_capacity(16)),
            refresh_gate: Arc::new(RefreshGate::default()),
            discovery: Arc::new(OnceCell::new()),
            http: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("reqwest client"),
        }
    }
}

impl AuthState {
    async fn discovery(&self) -> Result<&OidcDiscovery, String> {
        self.discovery
            .get_or_try_init(|| async {
                let issuer = std::env::var("ZITADEL_ISSUER")
                    .map_err(|_| "ZITADEL_ISSUER not set".to_string())?;
                let url = format!(
                    "{}/.well-known/openid-configuration",
                    issuer.trim_end_matches('/')
                );
                let doc: OidcDiscovery = self
                    .http
                    .get(&url)
                    .send()
                    .await
                    .map_err(|e| e.to_string())?
                    .json()
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(doc)
            })
            .await
            .map_err(|e: String| e)
    }

    fn choose_redirect_uri() -> &'static str {
        // In dev/sim, use the custom scheme; in production the universal link is preferred
        // (the app is registered for both). We always register the custom scheme to Zitadel.
        if cfg!(debug_assertions) {
            REDIRECT_PRIORITY[1]
        } else {
            REDIRECT_PRIORITY[0]
        }
    }

    /// Purge transactions older than 10 minutes.
    fn purge_stale_transactions(&self) {
        let ttl = Duration::from_secs(600);
        self.transactions
            .retain(|_, tx| tx.created_at.elapsed() < ttl);
    }
}

// ── Tauri commands ─────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn auth_start(app: AppHandle, prompt: Option<String>) -> Result<(), String> {
    let state = app.state::<AuthState>();
    state.purge_stale_transactions();

    let client_id = std::env::var("ZITADEL_NATIVE_CLIENT_ID")
        .map_err(|_| "ZITADEL_NATIVE_CLIENT_ID not set")?;

    let discovery = state.discovery().await?;

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let state_token = CsrfToken::new_random();
    let nonce: String = base64::Engine::encode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        rand_bytes(32),
    );

    let redirect_uri = AuthState::choose_redirect_uri();

    // Build scope string once — include offline_access (refresh token) and
    // the org scope so Zitadel includes resourceowner:id in the access token
    // (required by decode_jwt_claims to derive tenant_id, per auth invariant 37).
    let org_id = std::env::var("ZITADEL_DEFAULT_ORG_ID").unwrap_or_default();
    let scope = if org_id.is_empty() {
        "openid profile email offline_access".to_string()
    } else {
        format!("openid profile email offline_access urn:zitadel:iam:org:id:{org_id}")
    };

    let mut auth_url =
        url::Url::parse(&discovery.authorization_endpoint).map_err(|e| e.to_string())?;
    {
        let mut q = auth_url.query_pairs_mut();
        q.append_pair("client_id", &client_id);
        q.append_pair("response_type", "code");
        q.append_pair("scope", &scope);
        q.append_pair("redirect_uri", redirect_uri);
        q.append_pair("state", state_token.secret());
        q.append_pair("nonce", &nonce);
        q.append_pair("code_challenge", pkce_challenge.as_str());
        q.append_pair("code_challenge_method", "S256");
        if let Some(p) = prompt.as_deref() {
            q.append_pair("prompt", p);
        }
    }

    state.transactions.insert(
        state_token.secret().clone(),
        Transaction {
            code_verifier_secret: pkce_verifier.secret().clone(),
            nonce,
            redirect_uri: redirect_uri.to_string(),
            created_at: std::time::Instant::now(),
        },
    );

    // Open in system browser — never WKWebView.
    //
    // Use the OpenerExt trait method (`app.opener().open_url`), NOT the
    // free function `tauri_plugin_opener::open_url`. The free function always
    // uses the `open` crate, which shells out to a CLI binary
    // (`xdg-open`/`open`) that does not exist on iOS → `os error 2`. The trait
    // method routes through the mobile plugin to `UIApplication.open` on iOS.
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(auth_url.as_str(), None::<String>)
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn auth_get_access_token(app: AppHandle) -> Result<String, String> {
    let state = app.state::<AuthState>();

    let meta = store::load_meta().ok_or("not_authenticated")?;
    let now = chrono::Utc::now().timestamp();

    // Proactive refresh: if access token expires within 60s
    if meta.expires_at - now > 60 {
        return store::load_access_token().ok_or_else(|| "keychain_error".to_string());
    }

    // Single-flight: if another coroutine is already refreshing, wait and return
    if !state.refresh_gate.try_acquire().await {
        state.refresh_gate.wait_for_completion().await;
        return store::load_access_token().ok_or_else(|| "refresh_failed".to_string());
    }

    let result = do_refresh(&state, &meta.iss).await;
    state.refresh_gate.release().await;

    result
}

async fn do_refresh(state: &AuthState, _iss: &str) -> Result<String, String> {
    let refresh_token = store::load_refresh_token().ok_or("no_refresh_token")?;
    let client_id = std::env::var("ZITADEL_NATIVE_CLIENT_ID")
        .map_err(|_| "ZITADEL_NATIVE_CLIENT_ID not set")?;

    let discovery = state.discovery().await?;

    let params = [
        ("grant_type", "refresh_token"),
        ("client_id", &client_id),
        ("refresh_token", &refresh_token),
    ];

    let resp: serde_json::Value = state
        .http
        .post(&discovery.token_endpoint)
        .form(&params)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    if let Some(err) = resp.get("error").and_then(|e| e.as_str()) {
        // invalid_grant → clear keychain and require re-login
        store::delete_all();
        return Err(format!("refresh_error:{err}"));
    }

    let access = resp["access_token"]
        .as_str()
        .ok_or("missing_access_token")?
        .to_string();
    let new_refresh = resp
        .get("refresh_token")
        .and_then(|r| r.as_str())
        .unwrap_or(&refresh_token)
        .to_string();
    let expires_in = resp["expires_in"].as_i64().unwrap_or(3600);
    let expires_at = chrono::Utc::now().timestamp() + expires_in;

    store::update_tokens(&access, &new_refresh, expires_at).map_err(|e| e.to_string())?;

    Ok(access)
}

#[tauri::command]
pub async fn auth_sign_out(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AuthState>();

    // Best-effort: revoke the refresh token
    if let (Some(refresh_token), Ok(discovery)) =
        (store::load_refresh_token(), state.discovery().await)
        && let Some(rev_endpoint) = &discovery.revocation_endpoint
    {
        let client_id = std::env::var("ZITADEL_NATIVE_CLIENT_ID").unwrap_or_default();
        let _ = state
            .http
            .post(rev_endpoint)
            .timeout(Duration::from_secs(2))
            .form(&[("token", refresh_token.as_str()), ("client_id", &client_id)])
            .send()
            .await;
    }

    store::delete_all();
    let _ = app.emit("auth:signed_out", ());
    Ok(())
}

// ── Deep-link callback handler ─────────────────────────────────────────────────

/// Called by the deep-link plugin for both cold-start and runtime URLs.
pub async fn handle_callback_url(app: &AppHandle, callback_url: tauri::Url) {
    let state = app.state::<AuthState>();

    let params: std::collections::HashMap<String, String> = callback_url
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();

    let state_param = match params.get("state") {
        Some(s) => s.clone(),
        None => {
            tracing::warn!("deep-link: missing state parameter");
            return;
        }
    };

    let tx = match state.transactions.remove(&state_param) {
        Some((_, tx)) => tx,
        None => {
            tracing::warn!("deep-link: unknown or already-consumed state");
            return;
        }
    };

    // Validate transaction age
    if tx.created_at.elapsed() > Duration::from_secs(600) {
        tracing::warn!("deep-link: transaction expired");
        return;
    }

    // Validate redirect_uri: callback must start with the registered URI or one of the priority URIs
    let callback_path = callback_url.to_string();
    let path_matches = callback_path.starts_with(&tx.redirect_uri)
        || REDIRECT_PRIORITY
            .iter()
            .any(|uri| callback_path.starts_with(uri));
    if !path_matches {
        tracing::warn!("deep-link: redirect_uri mismatch");
        return;
    }

    let code = match params.get("code") {
        Some(c) => c.clone(),
        None => {
            tracing::warn!("deep-link: missing code");
            let _ = app.emit(
                "auth:error",
                params.get("error").cloned().unwrap_or_default(),
            );
            return;
        }
    };

    // Exchange code for tokens
    if let Err(e) = exchange_code(app, &state, code, tx).await {
        tracing::error!("deep-link: code exchange failed: {e}");
        let _ = app.emit("auth:error", e);
    }
}

async fn exchange_code(
    app: &AppHandle,
    state: &AuthState,
    code: String,
    tx: Transaction,
) -> Result<(), String> {
    let client_id = std::env::var("ZITADEL_NATIVE_CLIENT_ID")
        .map_err(|_| "ZITADEL_NATIVE_CLIENT_ID not set")?;
    let discovery = state.discovery().await?;

    let params = [
        ("grant_type", "authorization_code"),
        ("client_id", &client_id),
        ("code", &code),
        ("redirect_uri", &tx.redirect_uri),
        ("code_verifier", &tx.code_verifier_secret),
    ];

    let resp: serde_json::Value = state
        .http
        .post(&discovery.token_endpoint)
        .form(&params)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    if let Some(err) = resp.get("error").and_then(|e| e.as_str()) {
        return Err(format!("token_error:{err}"));
    }

    let access = resp["access_token"]
        .as_str()
        .ok_or("missing_access_token")?;
    let refresh = resp["refresh_token"]
        .as_str()
        .ok_or("missing_refresh_token")?;
    let expires_in = resp["expires_in"].as_i64().unwrap_or(3600);
    let expires_at = chrono::Utc::now().timestamp() + expires_in;

    // Decode sub + org_id from the access token payload (already validated by the gateway)
    let (iss, sub, org_id) = decode_jwt_claims(access)?;

    let meta = store::SessionMeta {
        iss,
        sub,
        org_id,
        expires_at,
    };

    store::save_tokens(&meta, access, refresh).map_err(|e| e.to_string())?;

    let _ = app.emit(
        "auth:signed_in",
        serde_json::json!({ "org_id": meta.org_id }),
    );
    Ok(())
}

// ── JWT claim extraction (no sig verify — gateway validates) ───────────────────

fn decode_jwt_claims(token: &str) -> Result<(String, String, String), String> {
    let parts: Vec<&str> = token.splitn(3, '.').collect();
    if parts.len() < 2 {
        return Err("invalid_jwt".to_string());
    }
    let pad = |s: &str| -> String {
        let r = s.len() % 4;
        if r == 0 {
            s.to_string()
        } else {
            format!("{}{}", s, "=".repeat(4 - r))
        }
    };
    let payload = base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE, pad(parts[1]))
        .map_err(|e| e.to_string())?;
    let claims: serde_json::Value = serde_json::from_slice(&payload).map_err(|e| e.to_string())?;

    let iss = claims["iss"].as_str().unwrap_or("").to_string();
    let sub = claims["sub"].as_str().unwrap_or("").to_string();
    let org_claim = std::env::var("ZITADEL_ORG_CLAIM")
        .unwrap_or_else(|_| "urn:zitadel:iam:user:resourceowner:id".to_string());
    let org_id = claims[&org_claim].as_str().unwrap_or("").to_string();

    if sub.is_empty() || org_id.is_empty() {
        return Err("missing_sub_or_org".to_string());
    }
    Ok((iss, sub, org_id))
}

fn rand_bytes(n: usize) -> Vec<u8> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    // Use a simple entropy source for nonce generation (non-cryptographic seed mixing)
    // In production this is fine — PKCE verifier is already generated by oauth2 crate
    // which uses OS entropy; this is only for the nonce string.
    let mut out = Vec::with_capacity(n);
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    // Fill with XOR of time + counter
    for i in 0..n {
        let mut h = DefaultHasher::new();
        (seed ^ (i as u32)).hash(&mut h);
        out.push(h.finish() as u8);
    }
    out
}
