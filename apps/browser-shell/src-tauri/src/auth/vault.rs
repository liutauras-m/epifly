//! Refresh-token vault: an IOTA Stronghold snapshot in the app's local data
//! directory, keyed with Argon2 over a per-install random salt
//! (`tauri_plugin_stronghold::kdf`, salt file `vault.salt` next to the
//! snapshot). Only Rust touches it — the Stronghold JS API is not registered,
//! so no webview can read the refresh token.
//!
//! Limitation: with no user-entered passphrase the key material (salt +
//! built-in password) lives on the same disk as the snapshot. The vault keeps
//! the token out of plaintext files, webview storage and logs; it does not
//! stop an attacker who can already read this user's app data directory.

use std::path::Path;
use tauri_plugin_stronghold::{kdf::KeyDerivation, stronghold::Stronghold};

use super::oidc::Identity;

const VAULT_PASSWORD: &str = "com.conusai.browser/refresh-token-vault/v1";
const CLIENT: &[u8] = b"auth";
const REFRESH_TOKEN: &[u8] = b"refresh_token";
const IDENTITY: &[u8] = b"identity";

pub struct Vault {
    hold: Stronghold,
}

impl Vault {
    pub fn open(dir: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("cannot create vault directory: {e}"))?;
        let key = KeyDerivation::argon2(VAULT_PASSWORD, &dir.join("vault.salt"));
        let hold = Stronghold::new(dir.join("vault.hold"), key).map_err(|e| format!("cannot open vault: {e}"))?;
        Ok(Self { hold })
    }

    fn client(&self) -> Result<iota_stronghold::Client, String> {
        self.hold
            .get_client(CLIENT)
            .or_else(|_| self.hold.load_client(CLIENT))
            .or_else(|_| self.hold.create_client(CLIENT))
            .map_err(|e| format!("vault client: {e}"))
    }

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, String> {
        self.client()?.store().get(key).map_err(|e| format!("vault read: {e}"))
    }

    /// The stored session: refresh token and the identity it belongs to.
    pub fn load(&self) -> Result<Option<(String, Identity)>, String> {
        let (Some(rt), Some(id)) = (self.get(REFRESH_TOKEN)?, self.get(IDENTITY)?) else {
            return Ok(None);
        };
        let rt = String::from_utf8(rt).map_err(|_| "vault holds a malformed refresh token")?;
        let id: Identity = serde_json::from_slice(&id).map_err(|_| "vault holds a malformed identity")?;
        Ok(Some((rt, id)))
    }

    pub fn store(&self, refresh_token: &str, identity: &Identity) -> Result<(), String> {
        let store = self.client()?.store();
        let identity = serde_json::to_vec(identity).map_err(|e| e.to_string())?;
        store
            .insert(REFRESH_TOKEN.to_vec(), refresh_token.as_bytes().to_vec(), None)
            .map_err(|e| format!("vault write: {e}"))?;
        store
            .insert(IDENTITY.to_vec(), identity, None)
            .map_err(|e| format!("vault write: {e}"))?;
        self.hold.save().map_err(|e| format!("vault save: {e}"))
    }

    pub fn clear(&self) -> Result<(), String> {
        let store = self.client()?.store();
        store.delete(REFRESH_TOKEN).map_err(|e| format!("vault delete: {e}"))?;
        store.delete(IDENTITY).map_err(|e| format!("vault delete: {e}"))?;
        self.hold.save().map_err(|e| format!("vault save: {e}"))
    }
}
