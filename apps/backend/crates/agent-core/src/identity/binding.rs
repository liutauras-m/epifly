/// Tenant identity binding: maps (zitadel_issuer, zitadel_org_id) → tenant_id.
///
/// This is the single authoritative lookup that converts an OIDC org claim into
/// an application-level tenant identity. Email is never used for tenant routing.
///
/// Provisioning policy:
/// - `AUTH_AUTO_PROVISION_TENANTS=true` (dev/staging): create on first login.
/// - `AUTH_AUTO_PROVISION_TENANTS=false` (prod default): reject with 403.
use sqlx::PgPool;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BindingError {
    #[error("tenant not provisioned for org {0}")]
    NotProvisioned(String),

    #[error("tenant suspended: {0}")]
    Suspended(String),

    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TenantBinding {
    pub tenant_id: String,
    pub plan_tier: String,
    pub status: String,
}

/// Look up (or auto-provision) the tenant binding for the given org.
///
/// Returns `Ok(binding)` if allowed, `Err(BindingError::NotProvisioned)` if
/// auto-provision is off and no binding exists.
pub async fn resolve_tenant(
    db: &PgPool,
    issuer: &str,
    org_id: &str,
    sub: &str,
) -> Result<TenantBinding, BindingError> {
    // Fast path: binding exists
    let existing = sqlx::query_as::<_, TenantBinding>(
        "SELECT tenant_id, plan_tier, status FROM tenant_identity_bindings \
         WHERE zitadel_issuer = $1 AND zitadel_org_id = $2"
    )
    .bind(issuer)
    .bind(org_id)
    .fetch_optional(db)
    .await?;

    if let Some(b) = existing {
        if b.status == "suspended" {
            return Err(BindingError::Suspended(b.tenant_id));
        }
        return Ok(b);
    }

    // No binding — check provisioning policy
    let auto_provision = std::env::var("AUTH_AUTO_PROVISION_TENANTS")
        .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
        .unwrap_or(false);

    if !auto_provision {
        return Err(BindingError::NotProvisioned(org_id.to_string()));
    }

    // Auto-provision: use org_id as tenant_id for determinism across nodes
    let tenant_id = org_id.to_string();

    sqlx::query(
        "INSERT INTO tenant_identity_bindings \
         (tenant_id, zitadel_issuer, zitadel_org_id, plan_tier, status, created_by_sub) \
         VALUES ($1, $2, $3, 'free', 'active', $4) \
         ON CONFLICT (zitadel_issuer, zitadel_org_id) DO NOTHING"
    )
    .bind(&tenant_id)
    .bind(issuer)
    .bind(org_id)
    .bind(sub)
    .execute(db)
    .await?;

    // Re-read so we get the row (handles concurrent inserts gracefully)
    let b = sqlx::query_as::<_, TenantBinding>(
        "SELECT tenant_id, plan_tier, status FROM tenant_identity_bindings \
         WHERE zitadel_issuer = $1 AND zitadel_org_id = $2"
    )
    .bind(issuer)
    .bind(org_id)
    .fetch_one(db)
    .await?;

    Ok(b)
}
