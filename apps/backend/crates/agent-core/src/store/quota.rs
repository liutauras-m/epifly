//! Per-tenant storage quota service.
//!
//! Aggregates used bytes by listing all objects via the per-tenant client
//! (cached 60s). Plan limits: FREE 1 GiB, PRO 100 GiB, ENTERPRISE unlimited.

use crate::context::tenant::PlanTier;
use crate::store::tenant_storage::{TenantStorageFactory, plan_quota_bytes};
use moka::future::Cache;
use std::sync::Arc;
use tracing::instrument;

pub struct StorageQuotaService {
    /// Cache: tenant_id → used bytes (60s TTL)
    usage_cache: Cache<String, u64>,
    factory: Arc<TenantStorageFactory>,
}

impl StorageQuotaService {
    pub fn new(factory: Arc<TenantStorageFactory>) -> Arc<Self> {
        Arc::new(Self {
            usage_cache: Cache::builder()
                .max_capacity(4096)
                .time_to_live(std::time::Duration::from_secs(60))
                .build(),
            factory,
        })
    }

    /// Returns used bytes for a tenant (cached 60s).
    #[instrument(skip(self), fields(tenant_id))]
    pub async fn used_bytes(&self, tenant_id: &str) -> u64 {
        if let Some(cached) = self.usage_cache.get(tenant_id).await {
            return cached;
        }

        let total = match self.factory.for_tenant(tenant_id).await {
            Ok(storage) => match storage.list_all_tenant_objects().await {
                Ok(metas) => metas.iter().map(|m| m.size as u64).sum(),
                Err(e) => {
                    tracing::warn!(tenant_id, error = %e, "quota: list_all failed, defaulting to 0");
                    0
                }
            },
            Err(e) => {
                tracing::warn!(tenant_id, error = %e, "quota: for_tenant failed, defaulting to 0");
                0
            }
        };

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
