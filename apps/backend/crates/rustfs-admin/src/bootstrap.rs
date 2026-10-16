//! Declarative RustFS bootstrap — runs on every gateway start.
//!
//! Reconciles the shared bucket, encryption, versioning, lifecycle rules, CORS and
//! (only when explicitly configured) bucket notifications. All operations are idempotent.

use crate::RustFsAdminClient;
use anyhow::Result;
use tracing::{info, instrument, warn};

/// Bucket-notification webhook target. Notifications also need a target registered
/// on the RustFS server itself, which a managed S3 service does not expose, so they
/// are configured only when the operator supplies both values.
#[derive(Clone)]
pub struct NotificationTarget {
    pub webhook_url: String,
}

/// Configuration for the declarative bootstrap (built by the gateway's env contract).
#[derive(Clone)]
pub struct BootstrapConfig {
    pub enabled: bool,
    pub encryption: bool,
    pub versioning: bool,
    pub lifecycle: bool,
    pub cors_origins: Vec<String>,
    pub notification: Option<NotificationTarget>,
}

/// Lifecycle XML:
/// - uploads/tmp/* expire 24h
/// - exports/* expire 7d
/// - non-current tenants/* versions expire 90d
fn lifecycle_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<LifecycleConfiguration xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
  <Rule>
    <ID>expire-tmp-uploads</ID>
    <Status>Enabled</Status>
    <Filter><Prefix>uploads/tmp/</Prefix></Filter>
    <Expiration><Days>1</Days></Expiration>
  </Rule>
  <Rule>
    <ID>expire-exports</ID>
    <Status>Enabled</Status>
    <Filter><Prefix>exports/</Prefix></Filter>
    <Expiration><Days>7</Days></Expiration>
  </Rule>
  <Rule>
    <ID>expire-old-workspace-versions</ID>
    <Status>Enabled</Status>
    <Filter><Prefix>tenants/</Prefix></Filter>
    <NoncurrentVersionExpiration><NoncurrentDays>90</NoncurrentDays></NoncurrentVersionExpiration>
  </Rule>
</LifecycleConfiguration>"#
        .to_string()
}

/// Run the declarative bootstrap.
#[instrument(skip(client, cfg))]
pub async fn bootstrap_storage(client: &RustFsAdminClient, cfg: &BootstrapConfig) -> Result<()> {
    if !cfg.enabled {
        info!("storage bootstrap disabled (RUSTFS_BOOTSTRAP=off)");
        return Ok(());
    }

    info!(bucket = %client.bucket, "starting declarative RustFS bootstrap");
    client.ensure_bucket().await?;
    info!("bucket ready");

    if cfg.encryption {
        match client.put_bucket_encryption().await {
            Ok(()) => info!("bucket default SSE-S3 encryption configured"),
            Err(e) => warn!(error = %e, "bucket encryption not configured"),
        }
    }

    if cfg.versioning {
        match client.set_versioning(true).await {
            Ok(()) => info!("versioning enabled"),
            Err(e) => warn!(error = %e, "versioning not configured"),
        }
    }

    if cfg.lifecycle {
        match client.put_lifecycle(&lifecycle_xml()).await {
            Ok(()) => info!("lifecycle rules configured"),
            Err(e) => warn!(error = %e, "lifecycle not configured"),
        }
    }

    if !cfg.cors_origins.is_empty() {
        match client.put_cors(&cfg.cors_origins).await {
            Ok(()) => info!(origins = ?cfg.cors_origins, "CORS configured"),
            Err(e) => warn!(error = %e, "CORS not configured"),
        }
    }

    match &cfg.notification {
        Some(target) => match client.put_bucket_notification(&target.webhook_url).await {
            Ok(()) => info!(url = %target.webhook_url, "bucket notifications configured"),
            Err(e) => warn!(error = %e, "bucket notifications not configured"),
        },
        None => info!("bucket notifications not configured — event-driven indexing is off"),
    }

    info!("declarative RustFS bootstrap complete");
    Ok(())
}
