//! Declarative RustFS bootstrap — runs on every gateway start.
//!
//! Reconciles bucket, versioning, lifecycle rules, CORS, and notifications.
//! All operations are idempotent. Controlled by `RUSTFS_BOOTSTRAP=on|off`.

use crate::RustFsAdminClient;
use anyhow::Result;
use tracing::{info, instrument, warn};

/// Configuration for the declarative bootstrap.
pub struct BootstrapConfig {
    pub versioning: bool,
    pub lifecycle: bool,
    pub cors_origins: Vec<String>,
    pub notification_webhook_url: Option<String>,
    pub notification_secret: Option<String>,
}

impl BootstrapConfig {
    pub fn from_env() -> Self {
        let web_origin = std::env::var("WEB_ORIGIN")
            .unwrap_or_else(|_| "http://localhost:3000,http://localhost:5173,https://tauri.localhost,tauri://localhost".into());
        let rustfs_origin =
            std::env::var("S3_ENDPOINT").unwrap_or_else(|_| "http://rustfs:9000".into());

        let mut origins: Vec<String> = web_origin
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        origins.push(rustfs_origin);

        Self {
            versioning: std::env::var("RUSTFS_VERSIONING")
                .map(|v| v != "off")
                .unwrap_or(true),
            lifecycle: true,
            cors_origins: origins,
            notification_webhook_url: std::env::var("RUSTFS_NOTIFICATION_WEBHOOK_URL").ok(),
            notification_secret: std::env::var("RUSTFS_WEBHOOK_SECRET").ok(),
        }
    }
}

/// Lifecycle XML with the rules from the plan:
/// - uploads/tmp/* expire 24h
/// - exports/* expire 7d
/// - non-current workspaces/* expire 90d
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

/// Run declarative bootstrap. Skipped when `RUSTFS_BOOTSTRAP=off`.
#[instrument(skip(client, cfg))]
pub async fn bootstrap_storage(client: &RustFsAdminClient, cfg: &BootstrapConfig) -> Result<()> {
    if std::env::var("RUSTFS_BOOTSTRAP").as_deref() == Ok("off") {
        info!("RUSTFS_BOOTSTRAP=off — skipping declarative storage bootstrap");
        return Ok(());
    }

    info!(bucket = %client.bucket, "starting declarative RustFS bootstrap");

    client.ensure_bucket().await?;
    info!("bucket ready");

    if std::env::var("RUSTFS_SSE").as_deref() != Ok("off") {
        match client.put_bucket_encryption().await {
            Ok(()) => info!("bucket default SSE-S3 encryption configured"),
            Err(e) => {
                warn!(error = %e, "bucket encryption config skipped (RustFS may not support it yet)")
            }
        }
    }

    if cfg.versioning {
        match client.set_versioning(true).await {
            Ok(()) => info!("versioning enabled"),
            Err(e) => {
                warn!(error = %e, "versioning config skipped (RustFS may not support it yet)")
            }
        }
    }

    if cfg.lifecycle {
        match client.put_lifecycle(&lifecycle_xml()).await {
            Ok(()) => info!("lifecycle rules configured"),
            Err(e) => warn!(error = %e, "lifecycle config skipped"),
        }
    }

    if !cfg.cors_origins.is_empty() {
        match client.put_cors(&cfg.cors_origins).await {
            Ok(()) => info!(origins = ?cfg.cors_origins, "CORS configured"),
            Err(e) => warn!(error = %e, "CORS config skipped"),
        }
    }

    if let (Some(url), Some(secret)) = (&cfg.notification_webhook_url, &cfg.notification_secret)
        && std::env::var("RUSTFS_NOTIFICATIONS").as_deref() != Ok("off")
    {
        match client.put_bucket_notification(url, secret).await {
            Ok(()) => info!(url, "bucket notifications configured"),
            Err(e) => warn!(error = %e, "bucket notification config skipped"),
        }
    }

    info!("declarative RustFS bootstrap complete");
    Ok(())
}
