//! Server-side plan resolution.
//!
//! A tenant's plan tier comes from its Lago subscription (cached), never from a
//! token claim or a request. Without billing configured, or without an active
//! subscription, the tenant is on the Free plan. A Lago subscription webhook
//! invalidates the tenant's cache entry.

use agent_core::PlanTier;
use billing_core::{BillingError, BillingProvider, SubscriptionStatus};
use common::types::TenantId;
use moka::future::Cache;
use std::sync::Arc;
use std::time::Duration;
use tracing::warn;

const PLAN_TTL: Duration = Duration::from_secs(300);
/// Lookups that failed (Lago unreachable) are retried after this long; the tenant
/// is treated as Free meanwhile.
const FAILURE_TTL: Duration = Duration::from_secs(30);

/// Map a Lago plan code to a tier.
pub fn tier_for_plan_code(code: &str) -> PlanTier {
    match code {
        "pro" => PlanTier::Pro,
        "team" | "enterprise" => PlanTier::Enterprise,
        _ => PlanTier::Free,
    }
}

pub struct PlanResolver {
    billing: Option<Arc<dyn BillingProvider>>,
    plans: Cache<String, PlanTier>,
    failures: Cache<String, ()>,
}

impl PlanResolver {
    pub fn new(billing: Option<Arc<dyn BillingProvider>>) -> Self {
        Self {
            billing,
            plans: Cache::builder()
                .max_capacity(100_000)
                .time_to_live(PLAN_TTL)
                .build(),
            failures: Cache::builder()
                .max_capacity(100_000)
                .time_to_live(FAILURE_TTL)
                .build(),
        }
    }

    pub async fn resolve(&self, tenant_id: &str) -> PlanTier {
        let Some(billing) = &self.billing else {
            return PlanTier::Free;
        };
        if let Some(tier) = self.plans.get(tenant_id).await {
            return tier;
        }
        if self.failures.get(tenant_id).await.is_some() {
            return PlanTier::Free;
        }
        match billing.get_subscription(&TenantId::from(tenant_id)).await {
            Ok(sub) => {
                let tier = match sub.status {
                    SubscriptionStatus::Active | SubscriptionStatus::Trialing => {
                        tier_for_plan_code(&sub.plan_key)
                    }
                    _ => PlanTier::Free,
                };
                self.plans.insert(tenant_id.to_string(), tier.clone()).await;
                tier
            }
            Err(BillingError::SubscriptionNotFound(_)) => {
                self.plans
                    .insert(tenant_id.to_string(), PlanTier::Free)
                    .await;
                PlanTier::Free
            }
            Err(e) => {
                warn!(tenant_id, error = %e, "plan lookup failed — treating tenant as Free");
                self.failures.insert(tenant_id.to_string(), ()).await;
                PlanTier::Free
            }
        }
    }

    /// Forget the cached plan (called on Lago subscription webhooks).
    pub async fn invalidate(&self, tenant_id: &str) {
        self.plans.invalidate(tenant_id).await;
        self.failures.invalidate(tenant_id).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn no_billing_means_free() {
        let r = PlanResolver::new(None);
        assert_eq!(r.resolve("org-1").await, PlanTier::Free);
    }

    #[test]
    fn plan_codes() {
        assert_eq!(tier_for_plan_code("pro"), PlanTier::Pro);
        assert_eq!(tier_for_plan_code("team"), PlanTier::Enterprise);
        assert_eq!(tier_for_plan_code("enterprise"), PlanTier::Enterprise);
        assert_eq!(tier_for_plan_code("free"), PlanTier::Free);
        assert_eq!(tier_for_plan_code("anything"), PlanTier::Free);
    }
}
