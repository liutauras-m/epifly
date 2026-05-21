use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionStatus {
    #[default]
    Active,
    Trialing,
    PastDue,
    Canceled,
    Incomplete,
}

impl std::fmt::Display for SubscriptionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubscriptionStatus::Active => write!(f, "active"),
            SubscriptionStatus::Trialing => write!(f, "trialing"),
            SubscriptionStatus::PastDue => write!(f, "past_due"),
            SubscriptionStatus::Canceled => write!(f, "canceled"),
            SubscriptionStatus::Incomplete => write!(f, "incomplete"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub tenant_id: String,
    pub lago_customer_id: String,
    pub lago_subscription_id: Option<String>,
    pub plan_key: String,
    pub status: SubscriptionStatus,
    pub current_period_start: Option<DateTime<Utc>>,
    pub current_period_end: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutSession {
    pub url: String,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invoice {
    pub id: String,
    pub tenant_id: String,
    pub amount_cents: i64,
    pub currency: String,
    pub status: String,
    pub issued_at: Option<DateTime<Utc>>,
    pub download_url: Option<String>,
}

/// A plan as defined in our catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanDefinition {
    pub key: String,
    pub display_name: String,
    pub monthly_price_cents: i64,
    pub currency: String,
    /// Max agent turns per day (None = unlimited).
    pub max_turns_per_day: Option<u64>,
    /// Max capability invocations per day.
    pub max_invokes_per_day: Option<u64>,
    /// Max storage in GB.
    pub max_storage_gb: Option<u64>,
    /// Max tokens per request.
    pub max_tokens: u64,
    /// Requests per minute.
    pub rate_limit_rpm: u32,
    /// Max agent rounds per request.
    pub max_turns: u32,
}
