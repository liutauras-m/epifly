use crate::error::BillingError;
use crate::events::UsageEvent;
use crate::types::{CheckoutSession, Invoice, Subscription};
use async_trait::async_trait;
use common::types::TenantId;

#[async_trait]
pub trait BillingProvider: Send + Sync + 'static {
    async fn create_or_update_subscription(
        &self,
        tenant_id: &TenantId,
        plan_key: &str,
        return_url: &str,
    ) -> Result<CheckoutSession, BillingError>;

    async fn cancel_subscription(&self, tenant_id: &TenantId) -> Result<(), BillingError>;

    async fn get_subscription(&self, tenant_id: &TenantId) -> Result<Subscription, BillingError>;

    async fn report_usage(&self, event: UsageEvent) -> Result<(), BillingError>;

    async fn list_invoices(&self, tenant_id: &TenantId) -> Result<Vec<Invoice>, BillingError>;

    async fn portal_url(
        &self,
        tenant_id: &TenantId,
        return_url: &str,
    ) -> Result<String, BillingError>;

    /// Ensure this tenant has a Lago customer record. Creates one if absent.
    async fn ensure_customer(
        &self,
        tenant_id: &TenantId,
        email: Option<&str>,
    ) -> Result<String, BillingError>;

    /// Verify a Lago webhook signature. Returns the raw body if valid.
    fn verify_webhook(&self, payload: &[u8], signature: &str) -> Result<(), BillingError>;

    /// Add wallet credits to a tenant's Lago account (admin operation).
    async fn add_credits(
        &self,
        tenant_id: &str,
        amount_cents: i64,
        description: Option<&str>,
    ) -> Result<(), BillingError>;

    /// Return an aggregated analytics summary for the super-admin dashboard.
    async fn analytics_summary(&self) -> Result<serde_json::Value, BillingError>;

    /// Ensure all plan definitions exist in Lago (idempotent upsert at boot).
    async fn ensure_plans(&self, catalog: &crate::catalog::PlanCatalog)
    -> Result<(), BillingError>;
}
