//! HMAC-SHA256 session token verifier.
//!
//! The token is a `<payload_b64>.<sig_b64>` string where:
//! - `payload_b64` is URL-safe base64 (no padding) of a JSON `SessionUser`.
//! - `sig_b64` is URL-safe base64 (no padding) of `HMAC-SHA256(UI_SESSION_KEY, payload_b64)`.
//!
//! The token is issued client-side (browser-shell or SvelteKit web app).
//! Server never needs to know `JWT_SECRET` to issue these tokens.

use agent_core::{PlanTier, TenantContext};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD as B64};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::sync::OnceLock;

type HmacSha256 = Hmac<Sha256>;

pub const COOKIE_NAME: &str = "conusai_session";
pub const SESSION_HEADER: &str = "x-session-token";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionUser {
    pub name: String,
    pub plan: String,
    #[serde(default)]
    pub role: String,
    pub exp: i64,
}

impl SessionUser {
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
        let tenant_id = std::env::var("CONUSAI_UI_TENANT_ID").unwrap_or_else(|_| "dev".into());
        let mut ctx = TenantContext::new(
            tenant_id,
            Some(self.name.clone()),
            self.plan_tier(),
            workspace_root,
        );
        ctx.role = self.role.parse().unwrap_or(agent_core::UserRole::User);
        ctx
    }
}

fn signing_key() -> &'static [u8] {
    static KEY: OnceLock<Vec<u8>> = OnceLock::new();
    KEY.get_or_init(|| {
        std::env::var("UI_SESSION_KEY")
            .unwrap_or_else(|_| "conusai-foundry-dev-secret-change-me-32b".into())
            .into_bytes()
    })
}

/// Verify an HMAC-signed session token and return the decoded `SessionUser`.
/// Returns `None` on invalid signature, malformed token, or expired `exp`.
///
/// **Timing-safe**: the HMAC is always computed before any early return so that
/// malformed tokens and wrong-signature tokens take the same amount of time.
/// Without this, a missing `.` separator would return faster than a bad
/// signature, giving an attacker a timing oracle.
pub fn verify(token: &str) -> Option<SessionUser> {
    // Always run the HMAC computation to prevent a timing side-channel.
    // For malformed tokens (no '.') we hash the whole token as a dummy input
    // so the wall-clock time matches a real (but rejected) verification attempt.
    let (payload_b64, sig_b64) = match token.split_once('.') {
        Some(parts) => parts,
        None => {
            let mut dummy = HmacSha256::new_from_slice(signing_key()).ok()?;
            dummy.update(token.as_bytes());
            let _ = dummy.finalize();
            return None;
        }
    };
    let mut mac = HmacSha256::new_from_slice(signing_key()).ok()?;
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

/// Constant-time byte comparison to prevent timing-based signature oracle attacks.
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
