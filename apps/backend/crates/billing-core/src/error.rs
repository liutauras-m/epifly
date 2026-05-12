use thiserror::Error;

#[derive(Debug, Error)]
pub enum BillingError {
    #[error("lago API error: {0}")]
    Lago(String),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("webhook signature invalid")]
    InvalidSignature,

    #[error("webhook event already processed (idempotency)")]
    DuplicateEvent,

    #[error("subscription not found for tenant {0}")]
    SubscriptionNotFound(String),

    #[error("plan not found: {0}")]
    PlanNotFound(String),

    #[error("configuration error: {0}")]
    Config(String),
}
