/// OS keychain token storage via the `keyring` crate.
///
/// Split into three separate entries to enable targeted deletion and cleaner
/// rotation. All entries use the same service name with distinct account keys.
///
/// - `session_meta`: `{ iss, sub, org_id, expires_at }` as JSON
/// - `access_token`: raw Bearer token string
/// - `refresh_token`: raw refresh token string
use keyring::Entry;
use serde::{Deserialize, Serialize};

const SERVICE: &str = "app.epifly.client";
const KEY_META: &str = "session_meta";
const KEY_ACCESS: &str = "access_token";
const KEY_REFRESH: &str = "refresh_token";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub iss: String,
    pub sub: String,
    pub org_id: String,
    pub expires_at: i64,
}

pub fn save_tokens(
    meta: &SessionMeta,
    access_token: &str,
    refresh_token: &str,
) -> Result<(), keyring::Error> {
    Entry::new(SERVICE, KEY_META)?
        .set_password(&serde_json::to_string(meta).unwrap())?;
    Entry::new(SERVICE, KEY_ACCESS)?.set_password(access_token)?;
    Entry::new(SERVICE, KEY_REFRESH)?.set_password(refresh_token)?;
    Ok(())
}

pub fn load_meta() -> Option<SessionMeta> {
    let raw = Entry::new(SERVICE, KEY_META).ok()?.get_password().ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn load_access_token() -> Option<String> {
    Entry::new(SERVICE, KEY_ACCESS).ok()?.get_password().ok()
}

pub fn load_refresh_token() -> Option<String> {
    Entry::new(SERVICE, KEY_REFRESH).ok()?.get_password().ok()
}

pub fn update_tokens(
    access_token: &str,
    refresh_token: &str,
    expires_at: i64,
) -> Result<(), keyring::Error> {
    // Update meta expires_at atomically with the new tokens
    if let Some(mut meta) = load_meta() {
        meta.expires_at = expires_at;
        Entry::new(SERVICE, KEY_META)?
            .set_password(&serde_json::to_string(&meta).unwrap())?;
    }
    Entry::new(SERVICE, KEY_ACCESS)?.set_password(access_token)?;
    Entry::new(SERVICE, KEY_REFRESH)?.set_password(refresh_token)?;
    Ok(())
}

pub fn delete_all() {
    // Best-effort cleanup — ignore errors (entry may not exist)
    let _ = Entry::new(SERVICE, KEY_META).and_then(|e| e.delete_credential());
    let _ = Entry::new(SERVICE, KEY_ACCESS).and_then(|e| e.delete_credential());
    let _ = Entry::new(SERVICE, KEY_REFRESH).and_then(|e| e.delete_credential());
}
