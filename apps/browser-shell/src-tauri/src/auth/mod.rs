//! Shell sign-in: Zitadel OIDC (authorization code + PKCE, `state`, `nonce`)
//! through the system browser and a loopback redirect, tokens held by Rust.
//!
//! - The refresh token lives only in the Stronghold vault (`vault.rs`).
//! - The access token lives in memory; the main window obtains it through
//!   `auth_access_token` to call the gateway with `Authorization: Bearer`.
//!   Tab webviews (external sites) are refused by every auth command.
//! - Refreshes are serialised by one async mutex, so concurrent callers share
//!   a single refresh-token grant.
//!
//! Build-time configuration (public values, also read by the Vite build):
//! `VITE_ZITADEL_ISSUER`, `VITE_ZITADEL_NATIVE_CLIENT_ID`, `VITE_GATEWAY_URL`.

pub mod oidc;
mod vault;

use std::sync::Mutex as StdMutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager, Runtime, State, Webview};
use tokio::sync::Mutex;

use oidc::{Discovery, Identity, Refresh};
use vault::Vault;

const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(300);
const REFRESH_SKEW_S: u64 = 60;
/// Label of the shell's own window — the only webview allowed to use auth commands.
const MAIN_WINDOW: &str = "main";

const ISSUER: Option<&str> = option_env!("VITE_ZITADEL_ISSUER");
const CLIENT_ID: Option<&str> = option_env!("VITE_ZITADEL_NATIVE_CLIENT_ID");
const GATEWAY_URL: Option<&str> = option_env!("VITE_GATEWAY_URL");

fn build_var(value: Option<&'static str>, name: &str) -> Result<&'static str, String> {
    value
        .filter(|v| !v.is_empty())
        .ok_or_else(|| format!("this build has no {name}; rebuild with it set"))
}

pub fn issuer() -> Result<&'static str, String> {
    build_var(ISSUER, "VITE_ZITADEL_ISSUER")
}

pub fn client_id() -> Result<&'static str, String> {
    build_var(CLIENT_ID, "VITE_ZITADEL_NATIVE_CLIENT_ID")
}

/// Public gateway origin the shell calls directly, e.g. `https://api.example.com`.
pub fn gateway_url() -> Result<&'static str, String> {
    build_var(GATEWAY_URL, "VITE_GATEWAY_URL").map(|u| u.trim_end_matches('/'))
}

fn now_s() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[derive(Default)]
struct Tokens {
    discovery: Option<Discovery>,
    access_token: Option<(String, u64)>,
    refresh_token: Option<String>,
    identity: Option<Identity>,
    loaded: bool,
}

pub struct AuthState {
    http: reqwest::Client,
    tokens: Mutex<Tokens>,
    vault: StdMutex<Option<Vault>>,
    signing_in: AtomicBool,
}

impl Default for AuthState {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthState {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .expect("HTTP client");
        Self { http, tokens: Mutex::new(Tokens::default()), vault: StdMutex::new(None), signing_in: AtomicBool::new(false) }
    }

    fn with_vault<T, R: Runtime>(
        &self,
        app: &AppHandle<R>,
        f: impl FnOnce(&Vault) -> Result<T, String>,
    ) -> Result<T, String> {
        let mut guard = self.vault.lock().map_err(|_| "vault lock poisoned")?;
        if guard.is_none() {
            let dir = app.path().app_local_data_dir().map_err(|e| format!("app data dir: {e}"))?;
            *guard = Some(Vault::open(&dir.join("auth"))?);
        }
        f(guard.as_ref().expect("vault opened above"))
    }

    async fn discovery(&self, tokens: &mut Tokens) -> Result<Discovery, String> {
        if let Some(d) = &tokens.discovery {
            return Ok(d.clone());
        }
        let d = oidc::discover(&self.http, issuer()?).await?;
        tokens.discovery = Some(d.clone());
        Ok(d)
    }

    fn load_from_vault<R: Runtime>(&self, app: &AppHandle<R>, tokens: &mut Tokens) -> Result<(), String> {
        if tokens.loaded {
            return Ok(());
        }
        if let Some((rt, identity)) = self.with_vault(app, |v| v.load())? {
            tokens.refresh_token = Some(rt);
            tokens.identity = Some(identity);
        }
        tokens.loaded = true;
        Ok(())
    }

    fn forget<R: Runtime>(&self, app: &AppHandle<R>, tokens: &mut Tokens) -> Result<(), String> {
        tokens.access_token = None;
        tokens.refresh_token = None;
        tokens.identity = None;
        self.with_vault(app, |v| v.clear())
    }

    /// The signed-in identity, from memory or the vault. No network.
    pub async fn identity<R: Runtime>(&self, app: &AppHandle<R>) -> Result<Option<Identity>, String> {
        let mut tokens = self.tokens.lock().await;
        self.load_from_vault(app, &mut tokens)?;
        Ok(tokens.refresh_token.as_ref().and(tokens.identity.clone()))
    }

    /// A currently valid access token, refreshing it when it is about to expire.
    pub async fn access_token<R: Runtime>(&self, app: &AppHandle<R>) -> Result<String, String> {
        let mut tokens = self.tokens.lock().await;
        if let Some((at, exp)) = &tokens.access_token
            && exp.saturating_sub(now_s()) > REFRESH_SKEW_S
        {
            return Ok(at.clone());
        }
        self.load_from_vault(app, &mut tokens)?;
        let Some(rt) = tokens.refresh_token.clone() else {
            return Err("not signed in".into());
        };
        let discovery = self.discovery(&mut tokens).await?;
        match oidc::refresh(&self.http, &discovery, client_id()?, &rt).await? {
            Refresh::Dead => {
                self.forget(app, &mut tokens)?;
                Err("session expired; sign in again".into())
            }
            Refresh::Tokens(t) => {
                let exp = now_s() + t.expires_in.unwrap_or(300);
                if let Some(new_rt) = t.refresh_token.filter(|n| n != &rt) {
                    let identity = tokens.identity.clone().ok_or("vault holds no identity")?;
                    self.with_vault(app, |v| v.store(&new_rt, &identity))?;
                    tokens.refresh_token = Some(new_rt);
                }
                tokens.access_token = Some((t.access_token.clone(), exp));
                Ok(t.access_token)
            }
        }
    }

    pub async fn sign_in<R: Runtime>(&self, app: &AppHandle<R>) -> Result<Identity, String> {
        if self.signing_in.swap(true, Ordering::SeqCst) {
            return Err("sign-in already in progress".into());
        }
        let result = self.sign_in_inner(app).await;
        self.signing_in.store(false, Ordering::SeqCst);
        result
    }

    async fn sign_in_inner<R: Runtime>(&self, app: &AppHandle<R>) -> Result<Identity, String> {
        let issuer = issuer()?;
        let client_id = client_id()?;
        let discovery = {
            let mut tokens = self.tokens.lock().await;
            self.discovery(&mut tokens).await?
        };

        // Bind first and keep the listener: no window between choosing a port and listening.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| format!("cannot open the sign-in callback port: {e}"))?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        let request = oidc::authorization_request(&discovery, client_id, port)?;

        open::that(&request.url).map_err(|e| format!("cannot open the system browser: {e}"))?;
        let code = oidc::await_callback(listener, &request.state, SIGN_IN_TIMEOUT).await?;
        let tokens = oidc::exchange_code(&self.http, &discovery, client_id, &request, &code).await?;

        let id_token = tokens.id_token.as_deref().ok_or("the identity provider returned no ID token")?;
        let identity = oidc::validate_id_token(id_token, issuer, client_id, &request.nonce, now_s())?;
        let refresh_token = tokens
            .refresh_token
            .ok_or("no refresh token; enable the refresh token grant on the Zitadel native app")?;

        self.with_vault(app, |v| v.store(&refresh_token, &identity))?;
        let mut state = self.tokens.lock().await;
        state.access_token = Some((tokens.access_token, now_s() + tokens.expires_in.unwrap_or(300)));
        state.refresh_token = Some(refresh_token);
        state.identity = Some(identity.clone());
        state.loaded = true;
        Ok(identity)
    }

    pub async fn sign_out<R: Runtime>(&self, app: &AppHandle<R>) -> Result<(), String> {
        let mut tokens = self.tokens.lock().await;
        self.load_from_vault(app, &mut tokens)?;
        let refresh_token = tokens.refresh_token.clone();
        self.forget(app, &mut tokens)?;
        let Ok(discovery) = self.discovery(&mut tokens).await else {
            return Ok(());
        };
        let client_id = client_id()?;
        if let Some(rt) = refresh_token {
            oidc::revoke(&self.http, &discovery, client_id, &rt).await;
        }
        // End the IdP browser session too, or the next sign-in is silent.
        if let Some(end_session) = &discovery.end_session_endpoint
            && let Ok(mut url) = reqwest::Url::parse(end_session)
        {
            url.query_pairs_mut().append_pair("client_id", client_id);
            let _ = open::that(url.as_str());
        }
        Ok(())
    }
}

fn require_main<R: Runtime>(webview: &Webview<R>) -> Result<(), String> {
    if webview.label() == MAIN_WINDOW {
        Ok(())
    } else {
        Err("auth commands are only available to the shell window".into())
    }
}

#[tauri::command]
pub async fn auth_status<R: Runtime>(
    app: AppHandle<R>,
    webview: Webview<R>,
    state: State<'_, AuthState>,
) -> Result<Option<Identity>, String> {
    require_main(&webview)?;
    state.identity(&app).await
}

#[tauri::command]
pub async fn auth_sign_in<R: Runtime>(
    app: AppHandle<R>,
    webview: Webview<R>,
    state: State<'_, AuthState>,
) -> Result<Identity, String> {
    require_main(&webview)?;
    state.sign_in(&app).await
}

#[tauri::command]
pub async fn auth_sign_out<R: Runtime>(
    app: AppHandle<R>,
    webview: Webview<R>,
    state: State<'_, AuthState>,
) -> Result<(), String> {
    require_main(&webview)?;
    state.sign_out(&app).await
}

/// Short-lived access token for the shell window's own gateway calls.
#[tauri::command]
pub async fn auth_access_token<R: Runtime>(
    app: AppHandle<R>,
    webview: Webview<R>,
    state: State<'_, AuthState>,
) -> Result<String, String> {
    require_main(&webview)?;
    state.access_token(&app).await
}
