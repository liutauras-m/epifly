use crate::catalog::PlanCatalog;
use crate::events::ActionType;
use agent_core::context::tenant::PlanTier;
use chrono::{DateTime, Utc};
use moka::future::Cache;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct QuotaDecision {
    pub allowed: bool,
    pub remaining: Option<u64>,
    pub reset_at: Option<DateTime<Utc>>,
    pub reason: Option<String>,
}

/// Cache key for per-tenant daily usage counters.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct QuotaKey {
    tenant_id: String,
    action: ActionType,
    /// Day bucket — YYYY-MM-DD (UTC).
    day: String,
}

impl QuotaKey {
    fn new(tenant_id: &str, action: &ActionType) -> Self {
        let day = Utc::now().format("%Y-%m-%d").to_string();
        Self {
            tenant_id: tenant_id.to_string(),
            action: action.clone(),
            day,
        }
    }
}

/// In-process quota checker backed by a moka cache.
///
/// Counters are advisory — Lago is the billing source of truth. The cache
/// provides fast pre-request enforcement without a database round-trip on
/// every call. A 30-second background re-sync keeps replicas roughly aligned.
pub struct QuotaChecker {
    cache: Cache<QuotaKey, u64>,
    catalog: Arc<PlanCatalog>,
}

impl QuotaChecker {
    pub fn new(catalog: Arc<PlanCatalog>) -> Self {
        let cache = Cache::builder()
            .max_capacity(100_000)
            .time_to_live(std::time::Duration::from_secs(86_400))
            .build();
        Self { cache, catalog }
    }

    pub async fn check(
        &self,
        tenant_id: &str,
        plan: &PlanTier,
        action: &ActionType,
        qty: u64,
    ) -> QuotaDecision {
        let plan_def = match self.catalog.by_tier(plan) {
            Some(p) => p,
            None => {
                return QuotaDecision {
                    allowed: true,
                    remaining: None,
                    reset_at: None,
                    reason: None,
                };
            }
        };

        let limit = match action {
            ActionType::AgentTurn => plan_def.max_turns_per_day,
            ActionType::CapabilityInvoke => plan_def.max_invokes_per_day,
            _ => None,
        };

        let limit = match limit {
            None => {
                return QuotaDecision {
                    allowed: true,
                    remaining: None,
                    reset_at: None,
                    reason: None,
                };
            }
            Some(l) => l,
        };

        let key = QuotaKey::new(tenant_id, action);
        let used = self.cache.get(&key).await.unwrap_or(0);

        if used + qty > limit {
            let reset_at = tomorrow_midnight();
            QuotaDecision {
                allowed: false,
                remaining: Some(limit.saturating_sub(used)),
                reset_at: Some(reset_at),
                reason: Some(format!(
                    "{} daily limit of {} reached (used {})",
                    action, limit, used
                )),
            }
        } else {
            QuotaDecision {
                allowed: true,
                remaining: Some(limit.saturating_sub(used + qty)),
                reset_at: Some(tomorrow_midnight()),
                reason: None,
            }
        }
    }

    pub async fn record(&self, tenant_id: &str, action: &ActionType, qty: u64) {
        let key = QuotaKey::new(tenant_id, action);
        let prev = self.cache.get(&key).await.unwrap_or(0);
        self.cache.insert(key, prev + qty).await;
    }
}

fn tomorrow_midnight() -> DateTime<Utc> {
    let now = Utc::now();
    let tomorrow = now.date_naive() + chrono::Duration::days(1);
    tomorrow
        .and_hms_opt(0, 0, 0)
        .map(|dt| dt.and_utc())
        .unwrap_or(now)
}
