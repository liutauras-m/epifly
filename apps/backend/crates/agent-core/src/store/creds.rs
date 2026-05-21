//! Per-tenant S3 credential storage.
//!
//! Credentials are encrypted with AES-256-GCM using the key from
//! `RUSTFS_IAM_ENC_KEY` (32 bytes, base64) before being written to redb.
//! Decrypted on demand and cached in an LRU (max 1024 entries, 5 min TTL).

use aes_gcm::{
    Aes256Gcm, Key, Nonce,
    aead::{Aead, AeadCore, KeyInit, OsRng},
};
use anyhow::{Context, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use moka::future::Cache;
use redb::{Database, TableDefinition};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::instrument;

const CREDS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("iam_tenant_creds");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageCreds {
    pub access_key: String,
    pub secret_key: String,
    /// Unix timestamp (seconds) when these credentials were created/rotated.
    #[serde(default)]
    pub created_at: i64,
    /// Per-tenant bucket name (`ws-{tenant_id}`). None → legacy shared `workspace` bucket.
    #[serde(default)]
    pub bucket: Option<String>,
}

/// Encrypted credential store backed by redb, with an in-process LRU cache.
pub struct CredentialStore {
    db: Arc<Database>,
    cipher: Aes256Gcm,
    cache: Cache<String, StorageCreds>,
}

impl CredentialStore {
    pub fn new(db: Arc<Database>) -> Result<Self> {
        let key_b64 = std::env::var("RUSTFS_IAM_ENC_KEY")
            .unwrap_or_else(|_| {
                // Dev-only fallback (32 zero bytes). Warn loudly in non-debug.
                if cfg!(not(debug_assertions)) {
                    tracing::error!(
                        "RUSTFS_IAM_ENC_KEY not set — per-tenant credentials will use an insecure dev key"
                    );
                }
                B64.encode([0u8; 32])
            });
        let raw = B64.decode(&key_b64).context("decode RUSTFS_IAM_ENC_KEY")?;
        if raw.len() != 32 {
            bail!("RUSTFS_IAM_ENC_KEY must decode to exactly 32 bytes");
        }
        let key = Key::<Aes256Gcm>::from_slice(&raw);
        let cipher = Aes256Gcm::new(key);

        Ok(Self {
            db,
            cipher,
            cache: Cache::builder()
                .max_capacity(1024)
                .time_to_live(std::time::Duration::from_secs(300))
                .build(),
        })
    }

    #[instrument(skip(self, creds), fields(tenant_id))]
    pub async fn store(&self, tenant_id: &str, creds: &StorageCreds) -> Result<()> {
        // Stamp created_at if the caller left it as zero.
        let mut creds = creds.clone();
        if creds.created_at == 0 {
            creds.created_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
        }
        let creds = &creds;
        let json = serde_json::to_vec(creds)?;

        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let mut ciphertext = self
            .cipher
            .encrypt(&nonce, json.as_slice())
            .map_err(|e| anyhow::anyhow!("encrypt creds: {e}"))?;

        // Prepend 12-byte nonce before ciphertext.
        let mut blob = nonce.to_vec();
        blob.append(&mut ciphertext);

        let key = format!("iam/tenant/{tenant_id}");
        let db = Arc::clone(&self.db);
        let blob_clone = blob.clone();
        let key_clone = key.clone();
        tokio::task::spawn_blocking(move || {
            let wtx = db.begin_write()?;
            {
                let mut table = wtx.open_table(CREDS_TABLE)?;
                table.insert(key_clone.as_str(), blob_clone.as_slice())?;
            }
            wtx.commit()?;
            Ok::<(), anyhow::Error>(())
        })
        .await??;

        self.cache
            .insert(tenant_id.to_string(), creds.clone())
            .await;
        Ok(())
    }

    #[instrument(skip(self), fields(tenant_id))]
    pub async fn load(&self, tenant_id: &str) -> Result<Option<StorageCreds>> {
        if let Some(cached) = self.cache.get(tenant_id).await {
            return Ok(Some(cached));
        }

        let key = format!("iam/tenant/{tenant_id}");
        let db = Arc::clone(&self.db);
        let key_clone = key.clone();
        let cipher = self.cipher.clone();

        let result = tokio::task::spawn_blocking(move || {
            let rtx = db.begin_read()?;
            let table = match rtx.open_table(CREDS_TABLE) {
                Ok(t) => t,
                Err(redb::TableError::TableDoesNotExist(_)) => return Ok(None),
                Err(e) => return Err(anyhow::anyhow!("open creds table: {e}")),
            };
            let entry = table.get(key_clone.as_str())?;
            match entry {
                None => Ok(None),
                Some(guard) => {
                    let blob = guard.value().to_vec();
                    if blob.len() < 12 {
                        bail!("creds blob too short");
                    }
                    let (nonce_bytes, ct) = blob.split_at(12);
                    let nonce = Nonce::from_slice(nonce_bytes);
                    let plain = cipher
                        .decrypt(nonce, ct)
                        .map_err(|e| anyhow::anyhow!("decrypt creds: {e}"))?;
                    let creds: StorageCreds = serde_json::from_slice(&plain)?;
                    Ok(Some(creds))
                }
            }
        })
        .await??;

        if let Some(ref c) = result {
            self.cache.insert(tenant_id.to_string(), c.clone()).await;
        }
        Ok(result)
    }

    pub async fn delete(&self, tenant_id: &str) -> Result<()> {
        self.cache.invalidate(tenant_id).await;
        let key = format!("iam/tenant/{tenant_id}");
        let db = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || {
            let wtx = db.begin_write()?;
            {
                let mut table = wtx.open_table(CREDS_TABLE)?;
                table.remove(key.as_str())?;
            }
            wtx.commit()?;
            Ok::<(), anyhow::Error>(())
        })
        .await??;
        Ok(())
    }

    /// List all tenant IDs that have stored credentials.
    pub async fn list_all_tenants(&self) -> Result<Vec<String>> {
        let db = Arc::clone(&self.db);
        tokio::task::spawn_blocking(move || {
            let rtx = db.begin_read()?;
            let table = match rtx.open_table(CREDS_TABLE) {
                Ok(t) => t,
                Err(redb::TableError::TableDoesNotExist(_)) => return Ok(vec![]),
                Err(e) => return Err(anyhow::anyhow!("open creds table: {e}")),
            };
            // All cred keys are "iam/tenant/{tenant_id}" — scan that prefix range.
            let range = table.range("iam/tenant/".."iam/tenant0")?;
            let mut ids = Vec::new();
            for item in range {
                let (k, _) = item?;
                let key = k.value().to_string();
                if let Some(id) = key.strip_prefix("iam/tenant/") {
                    ids.push(id.to_string());
                }
            }
            Ok(ids)
        })
        .await?
    }
}
