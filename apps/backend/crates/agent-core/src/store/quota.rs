//! Per-tenant storage quota service.
//!
//! Aggregates used bytes via `ListObjectsV2` (cached 60s in a moka cache).
//! Limits per plan tier: FREE 1 GiB, PRO 100 GiB, ENTERPRISE unlimited.

use crate::context::tenant::PlanTier;
use crate::store::creds::CredentialStore;
use crate::store::rustfs_content::build_root_store;
use moka::future::Cache;
use object_store::{ObjectStore, path::Path as OsPath};
use std::sync::Arc;
use tracing::instrument;

/// Per-plan quota in bytes (None = unlimited).
pub fn plan_quota_bytes(plan: &PlanTier) -> Option<u64> {
    match plan {
        PlanTier::Free => Some(1 * 1024 * 1024 * 1024),         // 1 GiB
        PlanTier::Pro => Some(100 * 1024 * 1024 * 1024),         // 100 GiB
        PlanTier::Enterprise => None,
    }
}

pub struct StorageQuotaService {
    /// Cache: tenant_id → used bytes (60s TTL)
    usage_cache: Cache<String, u64>,
    cred_store: Option<Arc<CredentialStore>>,
}

impl StorageQuotaService {
    pub fn new(cred_store: Option<Arc<CredentialStore>>) -> Arc<Self> {
        Arc::new(Self {
            usage_cache: Cache::builder()
                .max_capacity(4096)
                .time_to_live(std::time::Duration::from_secs(60))
                .build(),
            cred_store,
        })
    }

    /// Returns used bytes for a tenant (cached 60s).
    #[instrument(skip(self), fields(tenant_id))]
    pub async fn used_bytes(&self, tenant_id: &str) -> u64 {
        if let Some(cached) = self.usage_cache.get(tenant_id).await {
            return cached;
        }

        let store = build_root_store().unwrap_or_else(|_| panic!("root store"));
        let prefix = OsPath::from(format!("tenants/{tenant_id}/"));

        let mut total: u64 = 0;
        let mut stream = store.list(Some(&prefix));
        use futures::TryStreamExt;
        while let Ok(Some(meta)) = stream.try_next().await {
            total += meta.size as u64;
        }

        self.usage_cache.insert(tenant_id.to_string(), total).await;
        total
    }

    /// Returns `Err(quota_exceeded)` if adding `new_bytes` would exceed the plan quota.
    pub async fn check(
        &self,
        tenant_id: &str,
        plan: &PlanTier,
        new_bytes: u64,
    ) -> Result<(), QuotaError> {
        if !quota_enabled() {
            return Ok(());
        }
        let Some(limit) = plan_quota_bytes(plan) else {
            return Ok(());
        };
        let used = self.used_bytes(tenant_id).await;
        if used.saturating_add(new_bytes) > limit {
            Err(QuotaError {
                used,
                limit,
                requested: new_bytes,
            })
        } else {
            Ok(())
        }
    }

    pub fn invalidate(&self, tenant_id: &str) {
        let cache = self.usage_cache.clone();
        let tid = tenant_id.to_string();
        tokio::spawn(async move { cache.invalidate(&tid).await });
    }
}

fn quota_enabled() -> bool {
    std::env::var("RUSTFS_QUOTAS").as_deref() != Ok("off")
}

#[derive(Debug, thiserror::Error)]
#[error("storage quota exceeded: used={used}, limit={limit}, requested={requested}")]
pub struct QuotaError {
    pub used: u64,
    pub limit: u64,
    pub requested: u64,
}
