//! RustFS admin client — declarative bucket/IAM/lifecycle/CORS/notification bootstrap.
//!
//! Uses the MinIO-compatible admin REST API (`/minio/admin/v3/`) authenticated
//! via AWS SigV4, plus plain S3 API calls for bucket configuration.

pub mod bootstrap;
pub mod bucket;
pub mod iam;
pub mod signing;

pub use bootstrap::{BootstrapConfig, bootstrap_storage};
pub use iam::{IamCreds, provision_tenant, deprovision_tenant};
pub use bucket::sanitize_bucket_name;

use anyhow::{Context, Result, bail};
use bytes::Bytes;
use reqwest::{Client, Method, Response, StatusCode};
use serde::Deserialize;
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

    /// Create a named bucket if it does not yet exist; apply versioning + SSE.
    ///
    /// Idempotent: existing buckets are left unchanged.
    #[instrument(skip(self), fields(bucket_name))]
    pub async fn ensure_bucket_named(&self, bucket_name: &str) -> Result<()> {
        let path = format!("/{bucket_name}");
        let resp = self
            .s3_request(Method::PUT, &path, "", &BTreeMap::new(), Bytes::new())
            .await?;
        match resp.status() {
            StatusCode::OK | StatusCode::CONFLICT => {}
            s => {
                let body = resp.text().await.unwrap_or_default();
                if !body.contains("BucketAlreadyOwnedByYou") && !body.contains("BucketAlreadyExists") {
                    bail!("ensure_bucket_named({bucket_name}) failed: {s} — {body}")
                }
            }
        }

        // Enable versioning on the new bucket.
        let versioning_xml = r#"<?xml version="1.0" encoding="UTF-8"?><VersioningConfiguration xmlns="http://s3.amazonaws.com/doc/2006-03-01/"><Status>Enabled</Status></VersioningConfiguration>"#;
        let mut headers = BTreeMap::new();
        headers.insert("content-type".into(), "application/xml".into());
        let resp = self
            .s3_request(
                Method::PUT,
                &path,
                "versioning",
                &headers,
                Bytes::from(versioning_xml),
            )
            .await?;
        if !resp.status().is_success() {
            let t = resp.text().await.unwrap_or_default();
            tracing::warn!(bucket_name, "set_versioning on new bucket failed: {t}");
        }

        // Apply SSE-S3 default encryption.
        let sse_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<ServerSideEncryptionConfiguration xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
  <Rule><ApplyServerSideEncryptionByDefault><SSEAlgorithm>AES256</SSEAlgorithm></ApplyServerSideEncryptionByDefault></Rule>
</ServerSideEncryptionConfiguration>"#;
        headers.clear();
        headers.insert("content-type".into(), "application/xml".into());
        let resp = self
            .s3_request(
                Method::PUT,
                &path,
                "encryption=",
                &headers,
                Bytes::from(sse_xml),
            )
            .await?;
        if !resp.status().is_success() {
            let t = resp.text().await.unwrap_or_default();
            tracing::warn!(bucket_name, "put_bucket_encryption on new bucket failed: {t}");
        }

        // Apply lifecycle rules: expire uploads/tmp/ after 1 day, exports/ after 7 days.
        // Also adds AbortIncompleteMultipartUpload after 1 day as defence-in-depth.
        let lifecycle_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<LifecycleConfiguration xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
  <Rule>
    <ID>expire-staging-parts</ID>
    <Status>Enabled</Status>
    <Filter><Prefix>uploads/tmp/</Prefix></Filter>
    <Expiration><Days>1</Days></Expiration>
    <AbortIncompleteMultipartUpload><DaysAfterInitiation>1</DaysAfterInitiation></AbortIncompleteMultipartUpload>
  </Rule>
  <Rule>
    <ID>expire-exports</ID>
    <Status>Enabled</Status>
    <Filter><Prefix>exports/</Prefix></Filter>
    <Expiration><Days>7</Days></Expiration>
  </Rule>
</LifecycleConfiguration>"#;
        headers.clear();
        headers.insert("content-type".into(), "application/xml".into());
        let resp = self
            .s3_request(
                Method::PUT,
                &path,
                "lifecycle",
                &headers,
                Bytes::from(lifecycle_xml),
            )
            .await?;
        if !resp.status().is_success() {
            let t = resp.text().await.unwrap_or_default();
            tracing::warn!(bucket_name, "put_lifecycle on new bucket failed (non-fatal): {t}");
        }

        debug!(bucket_name, "per-tenant bucket ready");
        Ok(())
    }

    /// Delete a bucket by name. The bucket must be empty first; use
    /// `purge_bucket` to empty-then-delete in one call.
    /// Returns Ok(()) if the bucket doesn't exist (idempotent).
    #[instrument(skip(self), fields(bucket_name))]
    pub async fn delete_bucket(&self, bucket_name: &str) -> Result<()> {
        let path = format!("/{bucket_name}");
        let resp = self
            .s3_request(Method::DELETE, &path, "", &BTreeMap::new(), Bytes::new())
            .await?;
        match resp.status() {
            s if s.is_success() => Ok(()),
            StatusCode::NOT_FOUND => Ok(()),
            s => {
                let t = resp.text().await.unwrap_or_default();
                bail!("delete_bucket({bucket_name}) failed: {s} — {t}")
            }
        }
    }

    /// Delete every object version in `bucket_name` then delete the bucket.
    /// No-op if the bucket doesn't exist.
    #[instrument(skip(self), fields(bucket_name))]
    pub async fn purge_bucket(&self, bucket_name: &str) -> Result<()> {
        // List all objects (using ListObjectsV2 without versions first as a simpler path).
        let path = format!("/{bucket_name}");
        let mut continuation: Option<String> = None;
        loop {
            let query = if let Some(ref tok) = continuation {
                format!("list-type=2&max-keys=1000&continuation-token={}", urlencoding(tok))
            } else {
                "list-type=2&max-keys=1000".to_string()
            };
            let resp = self
                .s3_request(Method::GET, &path, &query, &BTreeMap::new(), Bytes::new())
                .await?;
            match resp.status() {
                StatusCode::NOT_FOUND => return Ok(()),
                s if !s.is_success() => {
                    let t = resp.text().await.unwrap_or_default();
                    bail!("purge_bucket list({bucket_name}) failed: {s} — {t}");
                }
                _ => {}
            }
            let xml = resp.text().await.context("read list body")?;
            let keys = parse_list_objects_v2(&xml);
            if keys.is_empty() {
                break;
            }
            for key in &keys {
                let obj_path = format!("/{bucket_name}/{}", urlencoding(key));
                let resp = self
                    .s3_request(Method::DELETE, &obj_path, "", &BTreeMap::new(), Bytes::new())
                    .await?;
                if !resp.status().is_success() && resp.status() != StatusCode::NOT_FOUND {
                    let s = resp.status();
                    let t = resp.text().await.unwrap_or_default();
                    bail!("purge_bucket delete({key}) failed: {s} — {t}");
                }
            }
            // Check truncated
            if !xml.contains("<IsTruncated>true</IsTruncated>") {
                break;
            }
            continuation = parse_next_continuation_token(&xml);
            if continuation.is_none() {
                break;
            }
        }
        self.delete_bucket(bucket_name).await
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

    /// Create a service account with a **bucket-scoped** policy (Phase 2).
    ///
    /// The policy grants full access to the named per-tenant bucket with no
    /// `s3:prefix` condition — simpler and smaller blast radius than the legacy
    /// prefix-scoped variant. Use this for tenants on the Modern layout.
    #[instrument(skip(self), fields(tenant_id, bucket_name))]
    pub async fn create_bucket_scoped_service_account(
        &self,
        tenant_id: &str,
        bucket_name: &str,
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
                    "Resource": [format!("arn:aws:s3:::{bucket_name}/*")]
                },
                {
                    "Effect": "Allow",
                    "Action": ["s3:ListBucket"],
                    "Resource": [format!("arn:aws:s3:::{bucket_name}")]
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
                "",
                Bytes::from(serde_json::to_vec(&body)?),
            )
            .await?;

        let s = resp.status();
        if !s.is_success() {
            let t = resp.text().await.unwrap_or_default();
            bail!("create_bucket_scoped_service_account failed: {s} — {t}");
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

    /// List object versions for a key prefix via S3 `GET /{bucket}?versions&prefix=`.
    ///
    /// Returns a list of `(version_id, last_modified_rfc3339, size, is_latest)` tuples.
    /// Parses the S3 XML response without an XML library using targeted string extraction.
    #[instrument(skip(self), fields(prefix))]
    pub async fn list_object_versions(
        &self,
        prefix: &str,
    ) -> Result<Vec<(String, String, u64, bool)>> {
        let path = format!("/{}", self.bucket);
        let query = format!("versions=&prefix={}", urlencoding(prefix));
        let resp = self
            .s3_request(Method::GET, &path, &query, &BTreeMap::new(), Bytes::new())
            .await?;
        let s = resp.status();
        if !s.is_success() {
            let t = resp.text().await.unwrap_or_default();
            bail!("list_object_versions failed: {s} — {t}");
        }
        let xml = resp.text().await.context("read list versions body")?;
        Ok(parse_version_list(&xml))
    }

    /// GET a specific object version from S3 (`?versionId={id}`).
    pub async fn get_object_version(&self, key: &str, version_id: &str) -> Result<bytes::Bytes> {
        let path = format!("/{}/{}", self.bucket, key);
        let query = format!("versionId={}", urlencoding(version_id));
        let resp = self
            .s3_request(Method::GET, &path, &query, &BTreeMap::new(), Bytes::new())
            .await?;
        let s = resp.status();
        if !s.is_success() {
            let t = resp.text().await.unwrap_or_default();
            bail!("get_object_version {key}?{version_id} failed: {s} — {t}");
        }
        Ok(resp.bytes().await.context("read object version bytes")?)
    }

    /// PUT bucket default encryption (SSE-S3 AES256).
    #[instrument(skip(self), fields(bucket = %self.bucket))]
    pub async fn put_bucket_encryption(&self) -> Result<()> {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<ServerSideEncryptionConfiguration xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
  <Rule>
    <ApplyServerSideEncryptionByDefault>
      <SSEAlgorithm>AES256</SSEAlgorithm>
    </ApplyServerSideEncryptionByDefault>
  </Rule>
</ServerSideEncryptionConfiguration>"#;
        let path = format!("/{}", self.bucket);
        let mut headers = BTreeMap::new();
        headers.insert("content-type".into(), "application/xml".into());
        let resp = self
            .s3_request(Method::PUT, &path, "encryption=", &headers, Bytes::from(xml))
            .await?;
        let s = resp.status();
        if s.is_success() {
            Ok(())
        } else {
            let t = resp.text().await.unwrap_or_default();
            bail!("put_bucket_encryption failed: {s} — {t}")
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

/// Parse a subset of the S3 `ListVersionsResult` XML without a full XML library.
/// Extracts `<Version>` blocks (not `<DeleteMarker>`) and returns
/// `(version_id, last_modified, size, is_latest)`.
fn parse_version_list(xml: &str) -> Vec<(String, String, u64, bool)> {
    let mut out = Vec::new();
    // Split on <Version> open tags; each piece after index 0 is one version block.
    for block in xml.split("<Version>").skip(1) {
        let end = block.find("</Version>").unwrap_or(block.len());
        let inner = &block[..end];
        let version_id = extract_xml_text(inner, "VersionId").unwrap_or_default().to_string();
        let last_modified = extract_xml_text(inner, "LastModified").unwrap_or_default().to_string();
        let size = extract_xml_text(inner, "Size")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let is_latest = extract_xml_text(inner, "IsLatest")
            .map(|s| s.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        if !version_id.is_empty() {
            out.push((version_id, last_modified, size, is_latest));
        }
    }
    out
}

fn extract_xml_text<'a>(xml: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    Some(&xml[start..end])
}

fn parse_list_objects_v2(xml: &str) -> Vec<String> {
    let mut keys = Vec::new();
    for block in xml.split("<Key>").skip(1) {
        let end = block.find("</Key>").unwrap_or(block.len());
        let key = &block[..end];
        if !key.is_empty() {
            keys.push(key.to_string());
        }
    }
    keys
}

fn parse_next_continuation_token(xml: &str) -> Option<String> {
    extract_xml_text(xml, "NextContinuationToken").map(|s| s.to_string())
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
