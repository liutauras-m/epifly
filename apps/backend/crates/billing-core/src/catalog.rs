use crate::types::PlanDefinition;
use agent_core::PlanTier;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Plan catalog — declarative plan definitions loaded at boot.
/// Override path: `CONUSAI_PLAN_CATALOG_PATH=/etc/conusai/plans.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanCatalog {
    pub plans: Vec<PlanDefinition>,
}

impl Default for PlanCatalog {
    fn default() -> Self {
        Self::built_in()
    }
}

impl PlanCatalog {
    /// Built-in plan catalog matching the payments-plan.md spec.
    pub fn built_in() -> Self {
        Self {
            plans: vec![
                PlanDefinition {
                    key: "free".into(),
                    display_name: "Free".into(),
                    monthly_price_cents: 0,
                    currency: "usd".into(),
                    max_turns_per_day: Some(50),
                    max_invokes_per_day: Some(10),
                    max_storage_gb: Some(1),
                    max_tokens: 4_096,
                    rate_limit_rpm: 10,
                    max_turns: 3,
                },
                PlanDefinition {
                    key: "pro".into(),
                    display_name: "Pro".into(),
                    monthly_price_cents: 2_000,
                    currency: "usd".into(),
                    max_turns_per_day: Some(500),
                    max_invokes_per_day: None,
                    max_storage_gb: Some(25),
                    max_tokens: 16_384,
                    rate_limit_rpm: 60,
                    max_turns: 8,
                },
                PlanDefinition {
                    key: "team".into(),
                    display_name: "Team".into(),
                    monthly_price_cents: 8_000,
                    currency: "usd".into(),
                    max_turns_per_day: Some(2_000),
                    max_invokes_per_day: None,
                    max_storage_gb: Some(100),
                    max_tokens: 32_768,
                    rate_limit_rpm: 120,
                    max_turns: 15,
                },
                PlanDefinition {
                    key: "enterprise".into(),
                    display_name: "Enterprise".into(),
                    monthly_price_cents: 0,
                    currency: "usd".into(),
                    max_turns_per_day: None,
                    max_invokes_per_day: None,
                    max_storage_gb: None,
                    max_tokens: 128_000,
                    rate_limit_rpm: 600,
                    max_turns: 20,
                },
            ],
        }
    }

    /// Load from TOML file at `CONUSAI_PLAN_CATALOG_PATH`, falling back to built-in.
    pub fn load() -> Self {
        let path = std::env::var("CONUSAI_PLAN_CATALOG_PATH").unwrap_or_default();
        if path.is_empty() {
            return Self::built_in();
        }
        match std::fs::read_to_string(&path) {
            Ok(contents) => match toml::from_str::<Self>(&contents) {
                Ok(catalog) => {
                    tracing::info!(path, "plan catalog loaded from file");
                    catalog
                }
                Err(e) => {
                    tracing::warn!(error = %e, path, "plan catalog parse failed — using built-in");
                    Self::built_in()
                }
            },
            Err(e) => {
                tracing::warn!(error = %e, path, "plan catalog file unreadable — using built-in");
                Self::built_in()
            }
        }
    }

    pub fn list(&self) -> &[PlanDefinition] {
        &self.plans
    }

    pub fn get(&self, key: &str) -> Option<&PlanDefinition> {
        self.plans.iter().find(|p| p.key == key)
    }

    pub fn by_tier(&self, tier: &PlanTier) -> Option<&PlanDefinition> {
        let key = match tier {
            PlanTier::Free => "free",
            PlanTier::Pro => "pro",
            PlanTier::Enterprise => "enterprise",
        };
        self.get(key)
    }

    pub fn as_map(&self) -> HashMap<String, &PlanDefinition> {
        self.plans.iter().map(|p| (p.key.clone(), p)).collect()
    }
}
