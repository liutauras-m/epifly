//! Mock session: HMAC-signed cookie carrying name + plan + expiry.
//!
//! In production, swap for OIDC; templates and routes don't change.

use agent_core::{PlanTier, TenantContext};
use axum::{
    extract::FromRequestParts,
    http::request::Parts,
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::CookieJar;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD as B64};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::sync::OnceLock;

pub const COOKIE_NAME: &str = "conusai_session";
const TTL_SECS: i64 = 24 * 3600;

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionUser {
    pub name: String,
    pub plan: String,
    #[serde(default)]
    pub role: String,
    pub exp: i64,
}

impl SessionUser {
    pub fn first_name(&self) -> &str {
        self.name.split_whitespace().next().unwrap_or(&self.name)
    }
    pub fn initials(&self) -> String {
        self.name
            .split_whitespace()
            .filter_map(|w| w.chars().next())
            .take(2)
            .collect::<String>()
            .to_uppercase()
    }
    pub fn plan_tier(&self) -> PlanTier {
        match self.plan.as_str() {
            "pro" => PlanTier::Pro,
            "enterprise" => PlanTier::Enterprise,
            _ => PlanTier::Free,
        }
    }
    pub fn tenant_context(&self) -> TenantContext {
        let workspace_root = std::env::var("CONUSAI_WORKSPACE_ROOT")
            .unwrap_or_else(|_| "/tmp/conusai/workspaces".into());
        // Single shared tenant in UI/dev mode so /v1/* (extract_tenant default = "dev")
        // and /ui/* (SessionUser-derived) share workspaces, threads, and ACL space.
        // In production, /v1/* requires JWT and tenant_id comes from claims; /ui/* is
        // not exposed there, so this fallback is dev-only.
        let tenant_id = std::env::var("CONUSAI_UI_TENANT_ID").unwrap_or_else(|_| "dev".into());
        let mut ctx = TenantContext::new(
            tenant_id,
            Some(self.name.clone()),
            self.plan_tier(),
            workspace_root,
        );
        ctx.role = agent_core::UserRole::from_str(&self.role);
        ctx
    }
}

fn key() -> &'static [u8] {
    static KEY: OnceLock<Vec<u8>> = OnceLock::new();
    KEY.get_or_init(|| {
        std::env::var("UI_SESSION_KEY")
            .unwrap_or_else(|_| "conusai-foundry-dev-secret-change-me-32b".into())
            .into_bytes()
    })
}

pub fn sign(name: &str, plan: &str, role: &str) -> String {
    let now = chrono::Utc::now().timestamp();
    let payload = SessionUser {
        name: name.to_string(),
        plan: plan.to_string(),
        role: role.to_string(),
        exp: now + TTL_SECS,
    };
    let json = serde_json::to_vec(&payload).unwrap();
    let payload_b64 = B64.encode(&json);
    let mut mac = HmacSha256::new_from_slice(key()).expect("HMAC key");
    mac.update(payload_b64.as_bytes());
    let sig = B64.encode(mac.finalize().into_bytes());
    format!("{payload_b64}.{sig}")
}

pub fn verify(cookie_value: &str) -> Option<SessionUser> {
    let (payload_b64, sig_b64) = cookie_value.split_once('.')?;
    let mut mac = HmacSha256::new_from_slice(key()).ok()?;
    mac.update(payload_b64.as_bytes());
    let expected = B64.encode(mac.finalize().into_bytes());
    if !ct_eq(expected.as_bytes(), sig_b64.as_bytes()) {
        return None;
    }
    let json = B64.decode(payload_b64).ok()?;
    let user: SessionUser = serde_json::from_slice(&json).ok()?;
    if user.exp < chrono::Utc::now().timestamp() {
        return None;
    }
    Some(user)
}

fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Axum extractor — auto-redirect to /login on missing/invalid cookie.
impl<S: Send + Sync> FromRequestParts<S> for SessionUser {
    type Rejection = Response;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_request_parts(parts, state)
            .await
            .map_err(|_| Redirect::to("/login").into_response())?;
        jar.get(COOKIE_NAME)
            .and_then(|c| verify(c.value()))
            .ok_or_else(|| Redirect::to("/login").into_response())
    }
}
