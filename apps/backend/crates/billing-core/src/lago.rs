use crate::error::BillingError;
use crate::events::UsageEvent;
use crate::provider::BillingProvider;
use crate::types::{CheckoutSession, Invoice, Subscription, SubscriptionStatus};
use async_trait::async_trait;
use common::types::TenantId;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, warn};

type HmacSha256 = Hmac<Sha256>;

// ── Config ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct LagoConfig {
    pub api_url: String,
    pub api_key: String,
    pub webhook_secret: String,
}

impl LagoConfig {
    pub fn from_env() -> Result<Self, BillingError> {
        let api_url = std::env::var("LAGO_API_URL")
            .unwrap_or_else(|_| "http://lago-api:3000".into());
        let api_key = std::env::var("LAGO_API_KEY").map_err(|_| {
            BillingError::Config("LAGO_API_KEY environment variable not set".into())
        })?;
        let webhook_secret = std::env::var("LAGO_WEBHOOK_SECRET").unwrap_or_default();
        Ok(Self { api_url, api_key, webhook_secret })
    }
}

// ── Internal Lago API types ───────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
struct LagoCustomer {
    external_id: String,
    name: Option<String>,
    email: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct LagoCreateCustomerRequest {
    customer: LagoCustomer,
}

#[derive(Debug, Serialize, Deserialize)]
struct LagoCreateCustomerResponse {
    customer: LagoCustomer,
}

#[derive(Debug, Serialize, Deserialize)]
struct LagoSubscription {
    external_id: Option<String>,
    external_customer_id: String,
    plan_code: String,
    status: Option<String>,
    started_at: Option<String>,
    ending_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct LagoCreateSubscriptionRequest {
    subscription: LagoSubscription,
}

#[derive(Debug, Serialize, Deserialize)]
struct LagoCreateSubscriptionResponse {
    subscription: LagoSubscription,
}

#[derive(Debug, Serialize, Deserialize)]
struct LagoEvent {
    transaction_id: String,
    external_customer_id: String,
    code: String,
    timestamp: i64,
    properties: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct LagoCreateEventRequest {
    event: LagoEvent,
}

#[derive(Debug, Serialize, Deserialize)]
struct LagoInvoice {
    lago_id: Option<String>,
    status: Option<String>,
    total_amount_cents: Option<i64>,
    currency: Option<String>,
    issuing_date: Option<String>,
    file_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct LagoInvoicesResponse {
    invoices: Vec<LagoInvoice>,
}

// ── Provider ──────────────────────────────────────────────────────────────────

pub struct LagoProvider {
    config: LagoConfig,
    client: reqwest::Client,
    event_queue: Arc<Mutex<Vec<UsageEvent>>>,
}

impl LagoProvider {
    pub fn new(config: LagoConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("HTTP client build failed");

        let provider = Self {
            config,
            client,
            event_queue: Arc::new(Mutex::new(Vec::new())),
        };

        // Spawn background flush loop.
        let queue = Arc::clone(&provider.event_queue);
        let flush_client = provider.client.clone();
        let flush_config = provider.config.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                let events: Vec<UsageEvent> = {
                    let mut q = queue.lock().await;
                    std::mem::take(&mut *q)
                };
                for event in events {
                    if let Err(e) = Self::flush_event(&flush_client, &flush_config, &event).await {
                        warn!(error = %e, transaction_id = %event.transaction_id, "usage event flush failed");
                    }
                }
            }
        });

        provider
    }

    pub fn from_env() -> Result<Self, BillingError> {
        let config = LagoConfig::from_env()?;
        Ok(Self::new(config))
    }

    fn base_url(&self) -> String {
        format!("{}/api/v1", self.config.api_url)
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.config.api_key)
    }

    async fn flush_event(
        client: &reqwest::Client,
        config: &LagoConfig,
        event: &UsageEvent,
    ) -> Result<(), BillingError> {
        let lago_event = LagoEvent {
            transaction_id: event.transaction_id.clone(),
            external_customer_id: event.lago_customer_id.clone(),
            code: event.action.to_string(),
            timestamp: event.timestamp.timestamp(),
            properties: event.properties.clone(),
        };

        let url = format!("{}/api/v1/events", config.api_url);
        let resp = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", config.api_key))
            .json(&LagoCreateEventRequest { event: lago_event })
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(BillingError::Lago(format!(
                "event post failed: HTTP {} — {}",
                status, body
            )));
        }
        debug!(
            transaction_id = %event.transaction_id,
            action = %event.action,
            "usage event flushed to Lago"
        );
        Ok(())
    }
}

#[async_trait]
impl BillingProvider for LagoProvider {
    async fn ensure_customer(
        &self,
        tenant_id: &TenantId,
        email: Option<&str>,
    ) -> Result<String, BillingError> {
        let external_id = tenant_id.to_string();

        // Try to get existing customer first.
        let url = format!("{}/customers/{}", self.base_url(), external_id);
        let resp = self
            .client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await?;

        if resp.status().is_success() {
            return Ok(external_id);
        }

        // Create customer.
        let create_url = format!("{}/customers", self.base_url());
        let body = LagoCreateCustomerRequest {
            customer: LagoCustomer {
                external_id: external_id.clone(),
                name: Some(external_id.clone()),
                email: email.map(String::from),
            },
        };

        let resp = self
            .client
            .post(&create_url)
            .header("Authorization", self.auth_header())
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(BillingError::Lago(format!(
                "create customer failed: HTTP {} — {}",
                status, text
            )));
        }

        Ok(external_id)
    }

    async fn create_or_update_subscription(
        &self,
        tenant_id: &TenantId,
        plan_key: &str,
        return_url: &str,
    ) -> Result<CheckoutSession, BillingError> {
        let external_customer_id = self.ensure_customer(tenant_id, None).await?;

        let url = format!("{}/subscriptions", self.base_url());
        let body = LagoCreateSubscriptionRequest {
            subscription: LagoSubscription {
                external_id: Some(format!("{}-{}", tenant_id, plan_key)),
                external_customer_id,
                plan_code: plan_key.to_string(),
                status: None,
                started_at: None,
                ending_at: None,
            },
        };

        let resp = self
            .client
            .post(&url)
            .header("Authorization", self.auth_header())
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(BillingError::Lago(format!(
                "create subscription failed: HTTP {} — {}",
                status, text
            )));
        }

        // For Stripe checkout, Lago returns the hosted page URL.
        // When Lago is configured with Stripe, the checkout URL comes from
        // the subscription response. Fall back to Lago customer portal.
        let portal_url = self.portal_url(tenant_id, return_url).await?;
        Ok(CheckoutSession {
            url: portal_url,
            expires_at: Some(chrono::Utc::now() + chrono::Duration::hours(1)),
        })
    }

    async fn cancel_subscription(&self, tenant_id: &TenantId) -> Result<(), BillingError> {
        let external_id = format!("{}-", tenant_id);
        // Fetch current subscription to find external_id.
        let url = format!("{}/subscriptions?external_customer_id={}", self.base_url(), tenant_id);
        let resp = self
            .client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(BillingError::SubscriptionNotFound(tenant_id.to_string()));
        }

        let delete_url = format!("{}/subscriptions/{}", self.base_url(), external_id);
        let resp = self
            .client
            .delete(&delete_url)
            .header("Authorization", self.auth_header())
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(BillingError::Lago(format!(
                "cancel subscription failed: HTTP {} — {}",
                status, text
            )));
        }
        Ok(())
    }

    async fn get_subscription(&self, tenant_id: &TenantId) -> Result<Subscription, BillingError> {
        let url = format!(
            "{}/subscriptions?external_customer_id={}",
            self.base_url(),
            tenant_id
        );
        let resp = self
            .client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(BillingError::SubscriptionNotFound(tenant_id.to_string()));
        }

        #[derive(Deserialize)]
        struct SubscriptionsResp {
            subscriptions: Vec<LagoSubscription>,
        }

        let data: SubscriptionsResp = resp.json().await?;
        let sub = data
            .subscriptions
            .into_iter()
            .next()
            .ok_or_else(|| BillingError::SubscriptionNotFound(tenant_id.to_string()))?;

        let status = match sub.status.as_deref() {
            Some("active") => SubscriptionStatus::Active,
            Some("pending") => SubscriptionStatus::Trialing,
            Some("terminated") => SubscriptionStatus::Canceled,
            _ => SubscriptionStatus::Active,
        };

        Ok(Subscription {
            tenant_id: tenant_id.to_string(),
            lago_customer_id: sub.external_customer_id,
            lago_subscription_id: sub.external_id,
            plan_key: sub.plan_code,
            status,
            current_period_start: sub
                .started_at
                .as_deref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc)),
            current_period_end: sub
                .ending_at
                .as_deref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc)),
        })
    }

    async fn report_usage(&self, event: UsageEvent) -> Result<(), BillingError> {
        let mut queue = self.event_queue.lock().await;
        queue.push(event);
        Ok(())
    }

    async fn list_invoices(&self, tenant_id: &TenantId) -> Result<Vec<Invoice>, BillingError> {
        let url = format!(
            "{}/invoices?external_customer_id={}",
            self.base_url(),
            tenant_id
        );
        let resp = self
            .client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await?;

        if !resp.status().is_success() {
            return Ok(vec![]);
        }

        let data: LagoInvoicesResponse = resp.json().await?;
        Ok(data
            .invoices
            .into_iter()
            .filter_map(|inv| {
                Some(Invoice {
                    id: inv.lago_id?,
                    tenant_id: tenant_id.to_string(),
                    amount_cents: inv.total_amount_cents.unwrap_or(0),
                    currency: inv.currency.unwrap_or_else(|| "usd".into()),
                    status: inv.status.unwrap_or_else(|| "draft".into()),
                    issued_at: inv
                        .issuing_date
                        .as_deref()
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| dt.with_timezone(&chrono::Utc)),
                    download_url: inv.file_url,
                })
            })
            .collect())
    }

    async fn portal_url(
        &self,
        tenant_id: &TenantId,
        _return_url: &str,
    ) -> Result<String, BillingError> {
        let url = format!(
            "{}/customers/{}/portal_url",
            self.base_url(),
            tenant_id
        );
        let resp = self
            .client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(BillingError::Lago(format!(
                "portal_url failed: HTTP {}",
                resp.status()
            )));
        }

        #[derive(Deserialize)]
        struct PortalResp {
            customer: PortalCustomer,
        }
        #[derive(Deserialize)]
        struct PortalCustomer {
            portal_url: Option<String>,
        }

        let data: PortalResp = resp.json().await?;
        data.customer
            .portal_url
            .ok_or_else(|| BillingError::Lago("no portal_url in response".into()))
    }

    fn verify_webhook(&self, payload: &[u8], signature: &str) -> Result<(), BillingError> {
        if self.config.webhook_secret.is_empty() {
            // If no secret configured, skip verification (dev mode).
            tracing::warn!("LAGO_WEBHOOK_SECRET not set — webhook signature verification skipped");
            return Ok(());
        }

        let mut mac =
            HmacSha256::new_from_slice(self.config.webhook_secret.as_bytes()).map_err(|e| {
                BillingError::Lago(format!("HMAC init error: {}", e))
            })?;
        mac.update(payload);
        let expected = hex::encode(mac.finalize().into_bytes());

        if !constant_time_eq(expected.as_bytes(), signature.as_bytes()) {
            return Err(BillingError::InvalidSignature);
        }
        Ok(())
    }

    async fn add_credits(
        &self,
        tenant_id: &str,
        amount_cents: i64,
        description: Option<&str>,
    ) -> Result<(), BillingError> {
        // Lago wallet transaction: grant prepaid credits.
        // Amount is in currency units (cents / 100 = dollars).
        let granted = format!("{:.2}", amount_cents as f64 / 100.0);
        let body = serde_json::json!({
            "wallet_transaction": {
                "wallet_id": tenant_id,
                "granted_credits": granted,
                "paid_credits": "0",
                "metadata": [{"key": "description", "value": description.unwrap_or("")}]
            }
        });

        let resp = self
            .client
            .post(format!("{}/api/v1/wallet_transactions", self.base_url()))
            .bearer_auth(&self.config.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| BillingError::Lago(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            return Err(BillingError::Lago(format!("add_credits {status}: {text}")));
        }
        Ok(())
    }

    async fn analytics_summary(&self) -> Result<serde_json::Value, BillingError> {
        // Fetch gross revenue analytics from Lago.
        let resp = self
            .client
            .get(format!("{}/api/v1/analytics/gross_revenue", self.base_url()))
            .bearer_auth(&self.config.api_key)
            .send()
            .await
            .map_err(|e| BillingError::Lago(e.to_string()))?;

        if resp.status().is_success() {
            let data: serde_json::Value = resp.json().await.map_err(|e| {
                BillingError::Lago(format!("analytics_summary deserialize: {e}"))
            })?;
            Ok(data)
        } else {
            Ok(serde_json::json!({ "configured": true, "data": [] }))
        }
    }

    async fn ensure_plans(&self, catalog: &crate::catalog::PlanCatalog) -> Result<(), BillingError> {
        for plan in catalog.list() {
            let code = &plan.key;
            // Check if plan already exists.
            let check = self
                .client
                .get(format!("{}/api/v1/plans/{code}", self.base_url()))
                .bearer_auth(&self.config.api_key)
                .send()
                .await
                .map_err(|e| BillingError::Lago(e.to_string()))?;

            if check.status().as_u16() == 200 {
                tracing::debug!(plan = code, "plan already exists in Lago, skipping");
                continue;
            }

            // Create plan.
            let body = serde_json::json!({
                "plan": {
                    "name": plan.display_name,
                    "code": code,
                    "interval": "monthly",
                    "amount_cents": plan.monthly_price_cents,
                    "amount_currency": "USD",
                    "pay_in_advance": false,
                    "charges": []
                }
            });

            let resp = self
                .client
                .post(format!("{}/api/v1/plans", self.base_url()))
                .bearer_auth(&self.config.api_key)
                .json(&body)
                .send()
                .await
                .map_err(|e| BillingError::Lago(e.to_string()))?;

            if resp.status().is_success() {
                tracing::info!(plan = code, "created plan in Lago");
            } else {
                let status = resp.status().as_u16();
                let text = resp.text().await.unwrap_or_default();
                tracing::warn!(plan = code, status, error = text, "ensure_plans upsert failed");
            }
        }
        Ok(())
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}
