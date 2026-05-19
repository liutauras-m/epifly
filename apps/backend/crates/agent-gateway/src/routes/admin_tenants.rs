//! Tenant lifecycle management for super-admins.
//!
//! `DELETE /admin/tenants/{id}` — permanently remove a tenant and all their data.
//!
//! Teardown order:
//!   1. Delete IAM service account from RustFS (revoke S3 access immediately).
//!   2. Purge S3 storage — per-tenant bucket (Phase 2) or legacy prefix objects.
//!   3. Delete credential record from `CredentialStore`.
//!   4. Purge workspace metadata (nodes, threads, messages, seeding flag) from redb.

use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use common::audit::AuditEvent;
use common::error::HttpError;
use rustfs_admin::iam::deprovision_tenant;
use std::sync::Arc;
use tracing::{info, warn};

/// DELETE /admin/tenants/{id}
///
/// Permanently destroys all data owned by the tenant. This action is irreversible.
pub async fn delete_tenant(
    State(state): State<Arc<AppState>>,
    Path(tenant_id): Path<String>,
) -> Result<impl IntoResponse, HttpError> {
    if tenant_id.is_empty() {
        return Err(HttpError::bad_request("tenant_id must not be empty"));
    }

    // ── Step 1: revoke IAM service account ──────────────────────────────
    if let (Some(admin), Some(cred_store)) = (&state.rustfs_admin, &state.cred_store) {
        match cred_store.load(&tenant_id).await {
            Ok(Some(creds)) => {
                if let Err(e) = deprovision_tenant(admin, &creds.access_key).await {
                    // Log but continue — key may already be gone.
                    warn!(tenant_id, error = %e, "failed to delete tenant IAM service account");
                }

                // ── Step 2: purge S3 storage ─────────────────────────────────────
                if let Some(bucket_name) = &creds.bucket {
                    // Phase 2: per-tenant bucket — delete all objects then the bucket.
                    if let Err(e) = admin.purge_bucket(bucket_name).await {
                        warn!(tenant_id, bucket = bucket_name, error = %e, "failed to purge per-tenant bucket");
                    } else {
                        info!(tenant_id, bucket = bucket_name, "deleted per-tenant bucket");
                    }
                } else {
                    // Phase 1 (legacy): objects under the tenant prefix in shared bucket.
                    // Cleanup is deferred to the migration job — see tenant_bucket_migration.rs.
                    warn!(
                        tenant_id,
                        "legacy prefix objects in shared bucket NOT deleted automatically \
                         — run migration job with MIGRATION_CLEANUP=true to purge"
                    );
                }
            }
            Ok(None) => {
                info!(tenant_id, "no credentials found — skipping IAM/storage teardown");
            }
            Err(e) => {
                warn!(tenant_id, error = %e, "failed to load tenant credentials for teardown");
            }
        }

        // ── Step 3: delete credential record ─────────────────────────────────
        if let Err(e) = cred_store.delete(&tenant_id).await {
            warn!(tenant_id, error = %e, "failed to delete tenant credentials from store");
        }
    }

    // ── Step 4: purge workspace metadata ────────────────────────────────
    if let Err(e) = state.workspace_store.purge_tenant_data(&tenant_id).await {
        warn!(tenant_id, error = %e, "failed to purge tenant workspace metadata");
    }

    // ── Audit ────────────────────────────────────────────────────────────
    let event = AuditEvent::new(&tenant_id, "admin.tenant.deleted")
        .with_metadata(serde_json::json!({ "deleted_by": "super_admin" }));
    if let Err(e) = state.audit_store.append(event).await {
        warn!(tenant_id, error = %e, "failed to write tenant deletion audit event");
    }

    info!(tenant_id, "tenant permanently deleted");
    Ok(StatusCode::NO_CONTENT)
}
