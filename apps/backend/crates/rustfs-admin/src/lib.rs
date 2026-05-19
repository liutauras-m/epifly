//! RustFS admin client — declarative bucket/IAM/lifecycle/CORS/notification bootstrap.
//!
//! Uses the MinIO-compatible admin REST API (`/minio/admin/v3/`) authenticated
//! via AWS SigV4, plus plain S3 API calls for bucket configuration.

pub mod bootstrap;
pub mod iam;
pub mod signing;

pub use bootstrap::{BootstrapConfig, bootstrap_storage};
pub use iam::{IamCreds, provision_tenant, deprovision_tenant};

use anyhow::{Context, Result, bail};
use bytes::Bytes;
use reqwest::{Client, Method, Response, StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tracing::{debug, instrument};

#[derive(Clone)]
pub struct RustFsAdminClient {
    http: Client,
    pub endpoint: String,
    pub access_key: String,
    pub secret_key: String,
    pub region: String,
    pub bucket: String,
}

impl RustFsAdminClient {
    pub fn new(
        endpoint: impl Into<String>,
        access_key: impl Into<String>,
        secret_key: impl Into<String>,
        bucket: impl Into<String>,
    ) -> Self {
        Self {
            http: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("reqwest client"),
            endpoint: endpoint.into(),
            access_key: access_key.into(),
            secret_key: secret_key.into(),
            region: "us-east-1".into(),
            bucket: bucket.into(),
        }
    }

    pub fn from_env() -> Self {
        let endpoint = std::env::var("S3_ENDPOINT")
            .unwrap_or_else(|_| "http://rustfs:9000".into());
        let access_key = std::env::var("RUSTFS_ROOT_ACCESS_KEY")
            .or_else(|_| std::env::var("AWS_ACCESS_KEY_ID"))
            .unwrap_or_else(|_| "rustfsadmin".into());
        let secret_key = std::env::var("RUSTFS_ROOT_SECRET_KEY")
            .or_else(|_| std::env::var("AWS_SECRET_ACCESS_KEY"))
            .unwrap_or_else(|_| "rustfsadmin".into());
        let bucket = std::env::var("S3_BUCKET")
            .unwrap_or_else(|_| "workspace".into());
        Self::new(endpoint, access_key, secret_key, bucket)
    }

    fn host(&self) -> String {
        self.endpoint
            .trim_start_matches("http://")
            .trim_start_matches("https://")
            .to_string()
    }

    fn is_https(&self) -> bool {
        self.endpoint.starts_with("https://")
    }

    /// Sign and send an S3 API request (bucket management operations).
    async fn s3_request(
        &self,
        method: Method,
        path: &str,
        query: &str,
        extra_headers: &BTreeMap<String, String>,
        body: Bytes,
    ) -> Result<Response> {
        let url = if query.is_empty() {
            format!("{}{}", self.endpoint, path)
        } else {
            format!("{}{}?{}", self.endpoint, path, query)
        };

        let signed = signing::sign_request(
            method.as_str(),
            path,
            query,
            &self.host(),
            &self.access_key,
            &self.secret_key,
            &self.region,
            "s3",
            extra_headers,
            &body,
        );

        let mut req = self.http.request(method, &url);
        for (k, v) in &signed {
            req = req.header(k, v);
        }
        if !body.is_empty() {
            req = req.body(body.to_vec());
        }

        let resp = req.send().await.context("S3 API request failed")?;
        Ok(resp)
    }

    /// Sign and send a MinIO admin API request.
    async fn admin_request(
        &self,
        method: Method,
        admin_path: &str,
        query: &str,
        body: Bytes,
    ) -> Result<Response> {
        let full_path = format!("/minio/admin/v3{admin_path}");
        self.s3_request(method, &full_path, query, &BTreeMap::new(), body).await
    }

    // ── S3 Bucket management ─────────────────────────────────────────────

    #[instrument(skip(self), fields(bucket = %self.bucket))]
    pub async fn ensure_bucket(&self) -> Result<()> {
        let path = format!("/{}", self.bucket);
        let resp = self
            .s3_request(Method::PUT, &path, "", &BTreeMap::new(), Bytes::new())
            .await?;
        match resp.status() {
            StatusCode::OK | StatusCode::CONFLICT => {
                debug!(bucket = %self.bucket, "bucket ready");
                Ok(())
            }
            s => {
                let body = resp.text().await.unwrap_or_default();
                if body.contains("BucketAlreadyOwnedByYou") || body.contains("BucketAlreadyExists") {
                    debug!(bucket = %self.bucket, "bucket already exists");
                    return Ok(());
                }
                bail!("ensure_bucket failed: {s} — {body}")
            }
        }
    }

    #[instrument(skip(self), fields(bucket = %self.bucket, enabled))]
    pub async fn set_versioning(&self, enabled: bool) -> Result<()> {
        let status = if enabled { "Enabled" } else { "Suspended" };
        let body = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><VersioningConfiguration xmlns="http://s3.amazonaws.com/doc/2006-03-01/"><Status>{status}</Status></VersioningConfiguration>"#
        );
        let path = format!("/{}", self.bucket);
        let mut headers = BTreeMap::new();
        headers.insert("content-type".into(), "application/xml".into());
        let resp = self
            .s3_request(Method::PUT, &path, "versioning", &headers, Bytes::from(body))
            .await?;
        let s = resp.status();
        if s.is_success() {
            Ok(())
        } else {
            let t = resp.text().await.unwrap_or_default();
            bail!("set_versioning failed: {s} — {t}")
        }
    }

    #[instrument(skip(self), fields(bucket = %self.bucket))]
    pub async fn put_lifecycle(&self, xml: &str) -> Result<()> {
        let path = format!("/{}", self.bucket);
        let mut headers = BTreeMap::new();
        headers.insert("content-type".into(), "application/xml".into());
        let resp = self
            .s3_request(
                Method::PUT,
                &path,
                "lifecycle",
                &headers,
                Bytes::from(xml.to_owned()),
            )
            .await?;
        let s = resp.status();
        if s.is_success() {
            Ok(())
        } else {
            let t = resp.text().await.unwrap_or_default();
            bail!("put_lifecycle failed: {s} — {t}")
        }
    }

    #[instrument(skip(self), fields(bucket = %self.bucket))]
    pub async fn put_cors(&self, origins: &[String]) -> Result<()> {
        let rules: String = origins
            .iter()
            .map(|o| {
                format!(
                    r#"<CORSRule>
                      <AllowedOrigin>{o}</AllowedOrigin>
                      <AllowedMethod>GET</AllowedMethod>
                      <AllowedMethod>PUT</AllowedMethod>
                      <AllowedMethod>HEAD</AllowedMethod>
                      <AllowedHeader>*</AllowedHeader>
                      <ExposeHeader>ETag</ExposeHeader>
                      <MaxAgeSeconds>3600</MaxAgeSeconds>
                    </CORSRule>"#
                )
            })
            .collect();
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><CORSConfiguration xmlns="http://s3.amazonaws.com/doc/2006-03-01/">{rules}</CORSConfiguration>"#
        );
        let path = format!("/{}", self.bucket);
        let mut headers = BTreeMap::new();
        headers.insert("content-type".into(), "application/xml".into());
        let resp = self
            .s3_request(
                Method::PUT,
                &path,
                "cors",
                &headers,
                Bytes::from(xml),
            )
            .await?;
        let s = resp.status();
        if s.is_success() {
            Ok(())
        } else {
            let t = resp.text().await.unwrap_or_default();
            bail!("put_cors failed: {s} — {t}")
        }
    }

    /// Configure a webhook notification target for all object events on this bucket.
    #[instrument(skip(self, webhook_url, _secret), fields(bucket = %self.bucket))]
    pub async fn put_bucket_notification(&self, webhook_url: &str, _secret: &str) -> Result<()> {
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<NotificationConfiguration xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
  <QueueConfiguration>
    <Id>rustfs-events</Id>
    <Queue>{webhook_url}</Queue>
    <Event>s3:ObjectCreated:*</Event>
    <Event>s3:ObjectRemoved:*</Event>
  </QueueConfiguration>
</NotificationConfiguration>"#
        );
        let path = format!("/{}", self.bucket);
        let mut headers = BTreeMap::new();
        headers.insert("content-type".into(), "application/xml".into());
        let resp = self
            .s3_request(
                Method::PUT,
                &path,
                "notification",
                &headers,
                Bytes::from(xml),
            )
            .await?;
        let s = resp.status();
        if s.is_success() {
            debug!("bucket notification configured → {webhook_url}");
            Ok(())
        } else {
            let t = resp.text().await.unwrap_or_default();
            // Not fatal — RustFS may not support this endpoint yet
            tracing::warn!("put_bucket_notification failed: {s} — {t}");
            Ok(())
        }
    }

    // ── MinIO admin API — IAM / service accounts ─────────────────────────

    /// Create a service account (access key) for the given IAM user with an
    /// inline policy restricting it to `tenants/{tenant_id}/*`.
    #[instrument(skip(self), fields(user))]
    pub async fn create_service_account(
        &self,
        user: &str,
        tenant_id: &str,
        bucket: &str,
    ) -> Result<(String, String)> {
        let policy = serde_json::json!({
            "Version": "2012-10-17",
            "Statement": [
                {
                    "Effect": "Allow",
                    "Action": [
                        "s3:GetObject", "s3:PutObject", "s3:DeleteObject",
                        "s3:AbortMultipartUpload", "s3:ListMultipartUploadParts",
                        "s3:CreateMultipartUpload", "s3:CompleteMultipartUpload",
                        "s3:UploadPart"
                    ],
                    "Resource": [
                        format!("arn:aws:s3:::{bucket}/tenants/{tenant_id}/*")
                    ]
                },
                {
                    "Effect": "Allow",
                    "Action": ["s3:ListBucket"],
                    "Resource": [format!("arn:aws:s3:::{bucket}")],
                    "Condition": {
                        "StringLike": {
                            "s3:prefix": [format!("tenants/{tenant_id}/*")]
                        }
                    }
                }
            ]
        });

        let body = serde_json::json!({
            "policy": policy.to_string(),
            "description": format!("tenant-{tenant_id}"),
        });

        let resp = self
            .admin_request(
                Method::PUT,
                "/add-service-account",
                &format!("user={}", urlencoding(user)),
                Bytes::from(serde_json::to_vec(&body)?),
            )
            .await?;

        let s = resp.status();
        if !s.is_success() {
            let t = resp.text().await.unwrap_or_default();
            bail!("create_service_account failed: {s} — {t}");
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ServiceAccountResp {
            access_key: String,
            secret_key: String,
        }

        let sa: ServiceAccountResp = resp
            .json()
            .await
            .context("parse service-account response")?;
        Ok((sa.access_key, sa.secret_key))
    }

    /// Delete a service account (access key).
    #[instrument(skip(self), fields(access_key))]
    pub async fn delete_service_account(&self, access_key: &str) -> Result<()> {
        let resp = self
            .admin_request(
                Method::DELETE,
                "/delete-service-account",
                &format!("accessKey={}", urlencoding(access_key)),
                Bytes::new(),
            )
            .await?;
        let s = resp.status();
        if s.is_success() || s == StatusCode::NOT_FOUND {
            Ok(())
        } else {
            let t = resp.text().await.unwrap_or_default();
            bail!("delete_service_account failed: {s} — {t}")
        }
    }

    /// List service accounts for a user.
    pub async fn list_service_accounts(&self, user: &str) -> Result<Vec<String>> {
        let resp = self
            .admin_request(
                Method::GET,
                "/list-service-accounts",
                &format!("user={}", urlencoding(user)),
                Bytes::new(),
            )
            .await?;
        let s = resp.status();
        if !s.is_success() {
            let t = resp.text().await.unwrap_or_default();
            bail!("list_service_accounts failed: {s} — {t}");
        }
        #[derive(Deserialize)]
        struct ListResp {
            accounts: Option<Vec<String>>,
        }
        let list: ListResp = resp.json().await.unwrap_or(ListResp { accounts: None });
        Ok(list.accounts.unwrap_or_default())
    }

    /// Set a named IAM policy (canned policy).
    #[instrument(skip(self, policy_json), fields(policy_name))]
    pub async fn put_policy(&self, policy_name: &str, policy_json: &str) -> Result<()> {
        let resp = self
            .admin_request(
                Method::PUT,
                "/add-canned-policy",
                &format!("name={}", urlencoding(policy_name)),
                Bytes::from(policy_json.to_owned()),
            )
            .await?;
        let s = resp.status();
        if s.is_success() {
            Ok(())
        } else {
            let t = resp.text().await.unwrap_or_default();
            bail!("put_policy failed: {s} — {t}")
        }
    }

    /// Attach a named policy to a user.
    #[instrument(skip(self), fields(policy_name, user))]
    pub async fn attach_policy(&self, policy_name: &str, user: &str) -> Result<()> {
        let resp = self
            .admin_request(
                Method::PUT,
                "/set-user-or-group-policy",
                &format!(
                    "policyName={}&userOrGroup={}&isGroup=false",
                    urlencoding(policy_name),
                    urlencoding(user)
                ),
                Bytes::new(),
            )
            .await?;
        let s = resp.status();
        if s.is_success() {
            Ok(())
        } else {
            let t = resp.text().await.unwrap_or_default();
            bail!("attach_policy failed: {s} — {t}")
        }
    }
}

fn urlencoding(s: &str) -> String {
    s.chars()
        .flat_map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '~' {
                vec![c]
            } else {
                format!("%{:02X}", c as u32).chars().collect()
            }
        })
        .collect()
}
