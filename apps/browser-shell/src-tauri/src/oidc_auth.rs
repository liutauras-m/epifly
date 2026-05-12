//! OIDC PKCE authentication for the Tauri desktop shell.
//!
//! Exposes two Tauri commands:
//! - `open_in_system_browser` — opens any URL in the OS default browser
//! - `pkce_login` — full PKCE flow: opens IdP in browser, waits for callback, returns auth code

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::net::TcpListener;
use tauri::command;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener as AsyncTcpListener;
use tracing::{info, warn};

/// Open a URL in the OS default browser (safe for Stripe Checkout, Zitadel login, etc.).
#[command]
pub async fn open_in_system_browser(url: String) -> Result<(), String> {
    open::that(&url).map_err(|e| format!("failed to open browser: {e}"))
}

/// PKCE login result returned to the frontend.
#[derive(serde::Serialize)]
pub struct PkceCallbackResult {
    pub code: String,
    pub state: Option<String>,
    pub redirect_uri: String,
}

/// Full PKCE login flow.
///
/// 1. Generates code_verifier + code_challenge
/// 2. Binds a local HTTP callback server on a random port
/// 3. Opens the IdP `auth_url` (with PKCE params appended) in the system browser
/// 4. Waits for the redirect callback carrying `?code=...`
/// 5. Returns the code + verifier so the frontend can exchange them server-side
///
/// The caller (frontend) should pass the code and verifier to the backend
/// `/v1/auth/zitadel/callback` endpoint for token exchange.
#[command]
pub async fn pkce_login(auth_url: String, extra_params: Option<String>) -> Result<serde_json::Value, String> {
    // Pick a random available port.
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("could not bind callback listener: {e}"))?;
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let redirect_uri = format!("http://127.0.0.1:{port}/callback");

    // Generate PKCE verifier + challenge.
    let verifier = generate_code_verifier();
    let challenge = code_challenge(&verifier);

    // Append PKCE + redirect params to the auth URL.
    let separator = if auth_url.contains('?') { '&' } else { '?' };
    let full_url = format!(
        "{auth_url}{separator}code_challenge={challenge}&code_challenge_method=S256\
         &redirect_uri={}&response_type=code{}",
        urlencoding::encode(&redirect_uri),
        extra_params.map(|p| format!("&{p}")).unwrap_or_default(),
    );

    info!(port, "starting PKCE callback listener");

    // Start local callback server before opening the browser.
    let async_listener = AsyncTcpListener::bind(format!("127.0.0.1:{port}"))
        .await
        .map_err(|e| format!("could not start callback server: {e}"))?;

    // Open the IdP in the system browser.
    open::that(&full_url).map_err(|e| format!("failed to open browser: {e}"))?;
    info!("opened PKCE auth URL in system browser");

    // Wait for the browser to redirect back.
    let (mut stream, _) = async_listener
        .accept()
        .await
        .map_err(|e| format!("callback accept error: {e}"))?;

    let mut reader = BufReader::new(&mut stream);
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .await
        .map_err(|e| format!("read error: {e}"))?;

    // Send a minimal success page.
    let body = "<html><body><h2>Login successful — you can close this tab.</h2></body></html>";
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    );
    if let Err(e) = stream.write_all(response.as_bytes()).await {
        warn!(error = %e, "could not write PKCE callback response");
    }

    // Parse code from "GET /callback?code=...&state=... HTTP/1.1"
    let query = request_line
        .split_whitespace()
        .nth(1)
        .and_then(|path| path.split_once('?'))
        .map(|(_, qs)| qs)
        .unwrap_or("");

    let params: std::collections::HashMap<_, _> = query
        .split('&')
        .filter_map(|kv| kv.split_once('='))
        .collect();

    let code = params
        .get("code")
        .map(|v| urlencoding::decode(v).unwrap_or_default().into_owned())
        .ok_or_else(|| "no code in PKCE callback".to_string())?;

    let state = params
        .get("state")
        .map(|v| urlencoding::decode(v).unwrap_or_default().into_owned());

    info!("PKCE callback received, code obtained");

    Ok(serde_json::json!({
        "code": code,
        "state": state,
        "redirect_uri": redirect_uri,
        "code_verifier": verifier,
    }))
}

fn generate_code_verifier() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn code_challenge(verifier: &str) -> String {
    let hash = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(hash)
}
