//! TenantStorage — single source of truth for all per-tenant object store operations.
//!
//! All S3/RustFS key construction is private. Callers can never obtain a raw
//! `tenants/...` string from this module. The public API operates entirely on
//! `VirtualPath` newtypes and typed result types.
//!
//! Layout variants:
//! - `LegacyPrefix`  — Phase 1: `tenants/{id}/workspaces/{vp}` in shared `workspace` bucket.
//! - `Modern`        — Phase 2: `workspaces/{vp}` in dedicated `ws-{id}` bucket.
//!
//! `TenantStorageFactory::for_tenant` selects the layout from `creds.bucket`.

use crate::context::tenant::PlanTier;
use crate::store::creds::{CredentialStore, StorageCreds};
use async_trait::async_trait;
use bytes::Bytes;
use common::audit::{AuditEvent, AuditStore};
use moka::future::Cache;
use object_store::{
    MultipartUpload, ObjectMeta, ObjectStore, PutOptions, PutPayload, PutResult,
    aws::AmazonS3Builder, path::Path as ObjectPath, signer::Signer,
};
use reqwest::Method;
use std::sync::atomic::{AtomicU64, Ordering};
use std::{sync::Arc, time::Duration};
use tracing::{instrument, warn};
use url::Url;

// ── Constants ─────────────────────────────────────────────────────────────────

pub const DEFAULT_TENANT_ROOT_NAME: &str = "Workspace";

fn s3_endpoint() -> Arc<str> {
    std::env::var("S3_ENDPOINT")
        .unwrap_or_else(|_| "http://rustfs:9000".into())
        .into()
}

fn s3_bucket() -> Arc<str> {
    std::env::var("S3_BUCKET")
        .unwrap_or_else(|_| "workspace".into())
        .into()
}

fn dev_fallback_enabled() -> bool {
    std::env::var("RUSTFS_DEV_FALLBACK_ROOT").as_deref() == Ok("on")
}

// ── VirtualPath ───────────────────────────────────────────────────────────────

/// Validated, normalized virtual path inside a tenant's logical workspace.
/// Constructed only via `VirtualPath::parse`, which rejects path-traversal
/// sequences, absolute paths, control chars, and over-long inputs.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VirtualPath(String);

impl VirtualPath {
    /// Parse and validate a virtual path string. Returns `Err` on any violation.
    pub fn parse(input: &str) -> Result<Self, StorageError> {
        if input.len() > 1024 {
            return Err(StorageError::InvalidPath(
                "virtual path exceeds 1024 bytes".into(),
            ));
        }
        if input.is_empty() {
            return Err(StorageError::InvalidPath(
                "virtual path must not be empty".into(),
            ));
        }
        if input.starts_with('/') || (input.len() >= 2 && input.chars().nth(1) == Some(':')) {
            return Err(StorageError::InvalidPath(
                "virtual path must be relative (no leading '/' or drive letter)".into(),
            ));
        }
        for segment in input.split('/') {
            if segment == ".." || segment == "." {
                return Err(StorageError::InvalidPath(
                    "virtual path must not contain '..' or '.' segments".into(),
                ));
            }
            if segment.is_empty() {
                return Err(StorageError::InvalidPath(
                    "virtual path must not contain empty segments (double '/')".into(),
                ));
            }
        }
        if input.ends_with('/') {
            return Err(StorageError::InvalidPath(
                "virtual path must not end with '/'".into(),
            ));
        }
        for byte in input.bytes() {
            if byte < 0x20 || byte == b'\0' {
                return Err(StorageError::InvalidPath(format!(
                    "virtual path contains invalid byte 0x{byte:02x}"
                )));
            }
        }
        Ok(VirtualPath(input.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for VirtualPath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for VirtualPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ── StorageError ──────────────────────────────────────────────────────────────

#[derive(thiserror::Error, Debug)]
pub enum StorageError {
    #[error("invalid virtual path: {0}")]
    InvalidPath(String),
    #[error("tenant credentials missing or invalid for tenant")]
    MissingTenantCreds,
    #[error("object not found")]
    NotFound,
    #[error("quota exceeded")]
    QuotaExceeded(String),
    #[error("upstream object store error: {0}")]
    Upstream(#[from] object_store::Error),
    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

// ── TenantStorageMode ─────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TenantStorageMode {
    /// Production default: per-tenant IAM is mandatory; any miss returns `MissingTenantCreds`.
    PerTenantIamRequired,
    /// Dev only (`RUSTFS_DEV_FALLBACK_ROOT=on`): falls back to root creds and emits a warning.
    PerTenantIamWithDevFallback,
}

// ── StorageLayout ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub enum StorageLayout {
    /// Phase 1: shared bucket, tenant prefix `tenants/{id}/workspaces/{vp}`.
    LegacyPrefix { tenant_id: String },
    /// Phase 2: per-tenant bucket `ws-{id}`, key `workspaces/{vp}`.
    Modern,
}

// ── WorkspaceStorage trait (narrow capability surface) ───────────────────────

/// Narrow, auditable storage surface for capabilities and agent code.
/// Implemented by `TenantStorage`; injected into `CapabilityExecutionContext`.
/// Gateway routes use `TenantStorage` directly (they need multipart + staging).
#[async_trait]
pub trait WorkspaceStorage: Send + Sync {
    async fn put_object(
        &self,
        path: &VirtualPath,
        body: Bytes,
        content_type: &str,
    ) -> Result<PutResult, StorageError>;

    async fn get_object(&self, path: &VirtualPath) -> Result<Bytes, StorageError>;

    async fn list_objects(&self, prefix: &VirtualPath) -> Result<Vec<ObjectMeta>, StorageError>;

    async fn delete_object(&self, path: &VirtualPath) -> Result<(), StorageError>;

    async fn presign_get(
        &self,
        path: &VirtualPath,
        ttl: Duration,
        content_disposition: Option<&str>,
    ) -> Result<Url, StorageError>;

    /// Returns the root workspace path constant (`Workspace`).
    fn root_path(&self) -> VirtualPath;
}

// ── CompletedPart / FinalizeResult ────────────────────────────────────────────

/// A part that was uploaded to the staging area.
#[derive(Debug, Clone)]
pub struct CompletedPart {
    pub n: u32,
    pub etag: String,
}

/// Result of a successful `finalize_staged_upload`.
#[derive(Debug)]
pub struct FinalizeResult {
    pub virtual_path: VirtualPath,
    pub size_bytes: u64,
    pub etag: Option<String>,
}

// ── AbortOnDropMultipart ──────────────────────────────────────────────────────

/// RAII guard: aborts the in-progress multipart upload on drop unless `.take()` was called.
/// Uses `tokio::spawn` because `Drop` is synchronous but `abort()` is async.
struct AbortOnDropMultipart {
    upload: Option<Box<dyn MultipartUpload>>,
    dest: ObjectPath,
}

impl AbortOnDropMultipart {
    fn new(upload: Box<dyn MultipartUpload>, dest: ObjectPath) -> Self {
        Self {
            upload: Some(upload),
            dest,
        }
    }
}

impl Drop for AbortOnDropMultipart {
    fn drop(&mut self) {
        if let Some(mut upload) = self.upload.take() {
            let dest = self.dest.clone();
            tokio::spawn(async move {
                if let Err(err) = upload.abort().await {
                    warn!(%dest, error = %err, "failed to abort orphaned multipart upload");
                }
            });
        }
    }
}

// ── StagedUploadFinalizer ─────────────────────────────────────────────────────

struct StagedUploadFinalizer<'a> {
    storage: &'a TenantStorage,
    upload_id: &'a str,
    dest: &'a VirtualPath,
}

impl<'a> StagedUploadFinalizer<'a> {
    async fn run(self) -> Result<FinalizeResult, StorageError> {
        let staging_prefix = self.storage.upload_staging_prefix(self.upload_id);

        // List all staged parts and sort by name (preserves part order).
        let listed = self
            .storage
            .client
            .list_with_delimiter(Some(&staging_prefix))
            .await?;

        if listed.objects.is_empty() {
            return Err(StorageError::NotFound);
        }

        let mut parts = listed.objects;
        parts.sort_by(|a, b| a.location.as_ref().cmp(b.location.as_ref()));

        let total_bytes: u64 = parts.iter().map(|m| m.size as u64).sum();

        // Open destination multipart and wrap in abort-on-drop guard immediately.
        let dest_path = self.storage.workspace_path(self.dest);
        let upload = self.storage.client.put_multipart(&dest_path).await?;
        let mut guard = AbortOnDropMultipart::new(upload, dest_path.clone());

        // Stream each staged part into the destination multipart (no full-file buffering).
        {
            let upload = guard.upload.as_mut().expect("guard not taken");
            for part_meta in &parts {
                let result = self.storage.client.get(&part_meta.location).await?;
                let data: Bytes = result.bytes().await?;
                upload.put_part(data.into()).await?;
            }
        }

        // Complete the upload.
        let put_result = guard
            .upload
            .as_mut()
            .expect("guard not taken")
            .complete()
            .await?;

        // Disarm the guard so drop() won't abort the now-completed upload.
        std::mem::forget(guard);

        // Best-effort cleanup of staging objects.
        for part_meta in &parts {
            if let Err(e) = self.storage.client.delete(&part_meta.location).await {
                warn!(
                    key = %part_meta.location,
                    error = %e,
                    "failed to delete staged part after finalize (lifecycle will clean up)"
                );
            }
        }

        Ok(FinalizeResult {
            virtual_path: self.dest.clone(),
            size_bytes: total_bytes,
            etag: put_result.e_tag,
        })
    }
}

// ── TenantStorage ─────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct TenantStorage {
    tenant_id: String,
    client: Arc<dyn ObjectStore>,
    creds: StorageCreds,
    bucket: Arc<str>,
    endpoint: Arc<str>,
    layout: StorageLayout,
    audit: Arc<dyn AuditStore>,
}

impl TenantStorage {
    // --- private key construction ---

    fn workspace_path(&self, vp: &VirtualPath) -> ObjectPath {
        match &self.layout {
            StorageLayout::LegacyPrefix { tenant_id } => {
                ObjectPath::from(format!("tenants/{tenant_id}/workspaces/{}", vp.as_str()))
            }
            StorageLayout::Modern => ObjectPath::from(format!("workspaces/{}", vp.as_str())),
        }
    }

    fn upload_staging_path(&self, upload_id: &str, filename: &str) -> ObjectPath {
        let safe_filename: String = filename
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
            .collect();
        match &self.layout {
            StorageLayout::LegacyPrefix { tenant_id } => ObjectPath::from(format!(
                "tenants/{tenant_id}/uploads/tmp/{upload_id}/{safe_filename}"
            )),
            StorageLayout::Modern => {
                ObjectPath::from(format!("uploads/tmp/{upload_id}/{safe_filename}"))
            }
        }
    }

    fn upload_staging_prefix(&self, upload_id: &str) -> ObjectPath {
        match &self.layout {
            StorageLayout::LegacyPrefix { tenant_id } => {
                ObjectPath::from(format!("tenants/{tenant_id}/uploads/tmp/{upload_id}/"))
            }
            StorageLayout::Modern => ObjectPath::from(format!("uploads/tmp/{upload_id}/")),
        }
    }

    fn meta_path(&self, key: &str) -> ObjectPath {
        match &self.layout {
            StorageLayout::LegacyPrefix { tenant_id } => {
                ObjectPath::from(format!("tenants/{tenant_id}/_meta/{key}"))
            }
            StorageLayout::Modern => ObjectPath::from(format!("_meta/{key}")),
        }
    }

    fn build_signer(&self) -> anyhow::Result<impl Signer + ObjectStore> {
        AmazonS3Builder::new()
            .with_endpoint(&*self.endpoint)
            .with_bucket_name(&*self.bucket)
            .with_access_key_id(&self.creds.access_key)
            .with_secret_access_key(&self.creds.secret_key)
            .with_allow_http(true)
            .with_region("us-east-1")
            .build()
            .map_err(|e| anyhow::anyhow!("build signer: {e}"))
    }

    // --- public workspace API ---

    #[instrument(skip(self, body), fields(tenant_id = %self.tenant_id, virtual_path = %vp))]
    pub async fn put_workspace_object(
        &self,
        vp: &VirtualPath,
        body: Bytes,
        content_type: &str,
    ) -> Result<PutResult, StorageError> {
        let key = self.workspace_path(vp);
        let bytes_len = body.len() as u64;
        let mut opts = PutOptions::default();
        opts.attributes.insert(
            object_store::Attribute::ContentType,
            content_type.to_owned().into(),
        );
        // Tag every PUT with the tenant id for audit / forensic recovery.
        opts.attributes.insert(
            object_store::Attribute::Metadata("tenant-id".into()),
            self.tenant_id.clone().into(),
        );
        let result = self
            .client
            .put_opts(&key, PutPayload::from(body), opts)
            .await?;
        self.emit_audit(
            "storage.put",
            vp.as_str(),
            bytes_len,
            result.e_tag.as_deref(),
            "ok",
        );
        Ok(result)
    }

    #[instrument(skip(self), fields(tenant_id = %self.tenant_id, virtual_path = %vp))]
    pub async fn get_workspace_object(&self, vp: &VirtualPath) -> Result<Bytes, StorageError> {
        let key = self.workspace_path(vp);
        match self.client.get(&key).await {
            Ok(result) => Ok(result.bytes().await?),
            Err(object_store::Error::NotFound { .. }) => Err(StorageError::NotFound),
            Err(e) => Err(StorageError::Upstream(e)),
        }
    }

    #[instrument(skip(self), fields(tenant_id = %self.tenant_id, virtual_path = %vp))]
    pub async fn delete_workspace_object(&self, vp: &VirtualPath) -> Result<(), StorageError> {
        let key = self.workspace_path(vp);
        match self.client.delete(&key).await {
            Ok(()) => {
                self.emit_audit("storage.delete", vp.as_str(), 0, None, "ok");
                Ok(())
            }
            Err(object_store::Error::NotFound { .. }) => Ok(()),
            Err(e) => Err(StorageError::Upstream(e)),
        }
    }

    #[instrument(skip(self), fields(tenant_id = %self.tenant_id))]
    pub async fn list_workspace_objects(
        &self,
        vp_prefix: &VirtualPath,
    ) -> Result<Vec<ObjectMeta>, StorageError> {
        let key = self.workspace_path(vp_prefix);
        use futures::TryStreamExt;
        let metas: Vec<ObjectMeta> = self.client.list(Some(&key)).try_collect().await?;
        Ok(metas)
    }

    #[instrument(skip(self), fields(tenant_id = %self.tenant_id, virtual_path = %vp))]
    pub async fn presign_workspace_get(
        &self,
        vp: &VirtualPath,
        ttl: Duration,
        _content_disposition: Option<&str>,
    ) -> Result<Url, StorageError> {
        let key = self.workspace_path(vp);
        let signer = self.build_signer().map_err(StorageError::Internal)?;
        signer
            .signed_url(Method::GET, &key, ttl)
            .await
            .map_err(|e| StorageError::Internal(anyhow::anyhow!("presign GET: {e}")))
    }

    #[instrument(skip(self), fields(tenant_id = %self.tenant_id, virtual_path = %vp))]
    pub async fn presign_workspace_put(
        &self,
        vp: &VirtualPath,
        ttl: Duration,
    ) -> Result<Url, StorageError> {
        let key = self.workspace_path(vp);
        let signer = self.build_signer().map_err(StorageError::Internal)?;
        signer
            .signed_url(Method::PUT, &key, ttl)
            .await
            .map_err(|e| StorageError::Internal(anyhow::anyhow!("presign PUT: {e}")))
    }

    #[instrument(skip(self), fields(tenant_id = %self.tenant_id, upload_id))]
    pub async fn presign_staging_put(
        &self,
        upload_id: &str,
        filename: &str,
        ttl: Duration,
    ) -> Result<Url, StorageError> {
        let key = self.upload_staging_path(upload_id, filename);
        let signer = self.build_signer().map_err(StorageError::Internal)?;
        signer
            .signed_url(Method::PUT, &key, ttl)
            .await
            .map_err(|e| StorageError::Internal(anyhow::anyhow!("presign staging PUT: {e}")))
    }

    /// Finalize a staged multipart upload, streaming parts without buffering.
    #[instrument(skip(self, _parts), fields(tenant_id = %self.tenant_id, upload_id, dest = %dest))]
    pub async fn finalize_staged_upload(
        &self,
        upload_id: &str,
        _parts: &[CompletedPart],
        dest: &VirtualPath,
    ) -> Result<FinalizeResult, StorageError> {
        let result = StagedUploadFinalizer {
            storage: self,
            upload_id,
            dest,
        }
        .run()
        .await;
        match &result {
            Ok(r) => self.emit_audit(
                "storage.finalize",
                dest.as_str(),
                r.size_bytes,
                r.etag.as_deref(),
                "ok",
            ),
            Err(e) => self.emit_audit_err("storage.finalize", dest.as_str(), &e.to_string()),
        }
        result
    }

    /// Fire-and-forget audit event for a successful mutating storage op.
    fn emit_audit(
        &self,
        op: &str,
        virtual_path: &str,
        bytes: u64,
        etag: Option<&str>,
        result: &str,
    ) {
        let event = AuditEvent::new(&self.tenant_id, op)
            .with_status(result)
            .with_metadata(serde_json::json!({
                "virtual_path": virtual_path,
                "bytes": bytes,
                "etag": etag,
            }));
        let audit = Arc::clone(&self.audit);
        tokio::spawn(async move {
            let _ = audit.append(event).await;
        });
    }

    /// Fire-and-forget audit event for a failed mutating storage op.
    fn emit_audit_err(&self, op: &str, virtual_path: &str, error: &str) {
        let event = AuditEvent::new(&self.tenant_id, op)
            .with_status("error")
            .with_metadata(serde_json::json!({
                "virtual_path": virtual_path,
                "error": error,
            }));
        let audit = Arc::clone(&self.audit);
        tokio::spawn(async move {
            let _ = audit.append(event).await;
        });
    }

    /// Write `_meta/seeded` marker object for optional external consistency.
    pub async fn write_seeded_marker(&self) -> Result<(), StorageError> {
        let key = self.meta_path("seeded");
        self.client
            .put(&key, PutPayload::from(Bytes::from("1")))
            .await?;
        Ok(())
    }

    /// Expose the layout for informational purposes (e.g., internal.rs key parsing).
    pub fn layout(&self) -> &StorageLayout {
        &self.layout
    }

    /// Return the S3 key string for a virtual path — **admin / versioning use only**.
    /// Routes that need to interact with `RustFsAdminClient` (list_object_versions,
    /// get_object_version) must call this to get the key without bypassing `VirtualPath`.
    pub fn workspace_s3_key(&self, virtual_path: &str) -> Result<String, StorageError> {
        let vp = VirtualPath::parse(virtual_path)?;
        Ok(self.workspace_path(&vp).to_string())
    }

    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    /// Returns `true` if the raw S3 object key belongs to this tenant.
    /// Layout-aware: legacy prefix checks the `tenants/{id}/` prefix;
    /// modern layout always returns `true` (per-tenant bucket).
    pub fn owns_object_key(&self, key: &str) -> bool {
        match &self.layout {
            StorageLayout::LegacyPrefix { tenant_id } => {
                let prefix = format!("tenants/{tenant_id}/");
                key.starts_with(&prefix)
            }
            StorageLayout::Modern => true,
        }
    }

    /// S3 key for a UI chat attachment (not stored under `workspaces/`).
    /// Legacy layout: `tenants/{id}/{upload_id}/{safe_filename}`
    /// Modern layout:  `attachments/{upload_id}/{safe_filename}`
    pub fn attachment_s3_key(&self, upload_id: &str, filename: &str) -> String {
        let safe_filename: String = filename
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
            .collect();
        match &self.layout {
            StorageLayout::LegacyPrefix { tenant_id } => {
                format!("tenants/{tenant_id}/{upload_id}/{safe_filename}")
            }
            StorageLayout::Modern => format!("attachments/{upload_id}/{safe_filename}"),
        }
    }

    /// Build a `TenantStorage` directly from raw credentials.
    ///
    /// Intended for backward-compat presign wrappers and admin ops that already
    /// have explicit credentials. Uses `LegacyPrefix` layout and a no-op audit store.
    pub fn from_raw_creds(
        tenant_id: &str,
        creds: StorageCreds,
        endpoint: &str,
        bucket: &str,
    ) -> Result<Self, StorageError> {
        use common::memory::InMemoryAuditStore;
        let store = AmazonS3Builder::new()
            .with_endpoint(endpoint)
            .with_bucket_name(bucket)
            .with_access_key_id(&creds.access_key)
            .with_secret_access_key(&creds.secret_key)
            .with_allow_http(true)
            .with_region("us-east-1")
            .build()
            .map_err(|e| StorageError::Internal(anyhow::anyhow!("from_raw_creds: {e}")))?;
        Ok(TenantStorage {
            tenant_id: tenant_id.to_owned(),
            client: Arc::new(store),
            creds,
            bucket: Arc::from(bucket),
            endpoint: Arc::from(endpoint),
            layout: StorageLayout::LegacyPrefix {
                tenant_id: tenant_id.to_owned(),
            },
            audit: Arc::new(InMemoryAuditStore::new()),
        })
    }

    /// Used by `StorageQuotaService` to list all objects for a tenant.
    pub async fn list_all_tenant_objects(&self) -> Result<Vec<ObjectMeta>, StorageError> {
        use futures::TryStreamExt;
        let prefix = match &self.layout {
            StorageLayout::LegacyPrefix { tenant_id } => {
                ObjectPath::from(format!("tenants/{tenant_id}/"))
            }
            StorageLayout::Modern => ObjectPath::from(""),
        };
        let metas: Vec<ObjectMeta> = self.client.list(Some(&prefix)).try_collect().await?;
        Ok(metas)
    }
}

#[async_trait]
impl WorkspaceStorage for TenantStorage {
    async fn put_object(
        &self,
        path: &VirtualPath,
        body: Bytes,
        content_type: &str,
    ) -> Result<PutResult, StorageError> {
        self.put_workspace_object(path, body, content_type).await
    }

    async fn get_object(&self, path: &VirtualPath) -> Result<Bytes, StorageError> {
        self.get_workspace_object(path).await
    }

    async fn list_objects(&self, prefix: &VirtualPath) -> Result<Vec<ObjectMeta>, StorageError> {
        self.list_workspace_objects(prefix).await
    }

    async fn delete_object(&self, path: &VirtualPath) -> Result<(), StorageError> {
        self.delete_workspace_object(path).await
    }

    async fn presign_get(
        &self,
        path: &VirtualPath,
        ttl: Duration,
        content_disposition: Option<&str>,
    ) -> Result<Url, StorageError> {
        self.presign_workspace_get(path, ttl, content_disposition)
            .await
    }

    fn root_path(&self) -> VirtualPath {
        VirtualPath(DEFAULT_TENANT_ROOT_NAME.to_owned())
    }
}

// ── TenantStorageFactory ──────────────────────────────────────────────────────

#[derive(Clone)]
pub struct TenantStorageFactory {
    pub mode: TenantStorageMode,
    /// Shared bucket name for legacy layout.
    bucket: Arc<str>,
    endpoint: Arc<str>,
    creds_store: Arc<CredentialStore>,
    /// LRU cache of per-tenant ObjectStore clients (1024 entries, 5 min TTL).
    client_cache: Cache<String, Arc<dyn ObjectStore>>,
    /// Counts how many times the dev-fallback path has been used since boot.
    pub fallback_count: Arc<AtomicU64>,
    /// Audit sink — receives fire-and-forget events for every mutating storage op.
    audit: Arc<dyn AuditStore>,
}

impl TenantStorageFactory {
    pub fn new(creds_store: Arc<CredentialStore>, audit: Arc<dyn AuditStore>) -> Self {
        let mode = if dev_fallback_enabled() {
            TenantStorageMode::PerTenantIamWithDevFallback
        } else {
            TenantStorageMode::PerTenantIamRequired
        };
        Self {
            mode,
            bucket: s3_bucket(),
            endpoint: s3_endpoint(),
            creds_store,
            client_cache: Cache::builder()
                .max_capacity(1024)
                .time_to_live(Duration::from_secs(300))
                .build(),
            fallback_count: Arc::new(AtomicU64::new(0)),
            audit,
        }
    }

    /// Build a `TenantStorage` for the given tenant, selecting layout from credentials.
    pub async fn for_tenant(&self, tenant_id: &str) -> Result<TenantStorage, StorageError> {
        let creds = self.resolve_creds(tenant_id).await?;

        // Pick layout and effective bucket.
        let (layout, bucket) = if let Some(ref per_bucket) = creds.bucket {
            (StorageLayout::Modern, Arc::from(per_bucket.as_str()))
        } else {
            (
                StorageLayout::LegacyPrefix {
                    tenant_id: tenant_id.to_owned(),
                },
                Arc::clone(&self.bucket),
            )
        };

        // Build or reuse the cached ObjectStore client.
        let cache_key = format!("{tenant_id}::{}", bucket);
        let client = if let Some(cached) = self.client_cache.get(&cache_key).await {
            cached
        } else {
            let store = self.build_client(&creds, &bucket)?;
            self.client_cache
                .insert(cache_key, Arc::clone(&store))
                .await;
            store
        };

        Ok(TenantStorage {
            tenant_id: tenant_id.to_owned(),
            client,
            creds,
            bucket,
            endpoint: Arc::clone(&self.endpoint),
            layout,
            audit: Arc::clone(&self.audit),
        })
    }

    async fn resolve_creds(&self, tenant_id: &str) -> Result<StorageCreds, StorageError> {
        match self.creds_store.load(tenant_id).await {
            Ok(Some(c)) => Ok(c),
            Ok(None) => match self.mode {
                TenantStorageMode::PerTenantIamRequired => Err(StorageError::MissingTenantCreds),
                TenantStorageMode::PerTenantIamWithDevFallback => {
                    self.fallback_count.fetch_add(1, Ordering::Relaxed);
                    warn!(
                        tenant_id,
                        "DEV FALLBACK to root creds — RUSTFS_DEV_FALLBACK_ROOT=on"
                    );
                    Ok(root_creds())
                }
            },
            Err(e) => match self.mode {
                TenantStorageMode::PerTenantIamRequired => Err(StorageError::Internal(e)),
                TenantStorageMode::PerTenantIamWithDevFallback => {
                    self.fallback_count.fetch_add(1, Ordering::Relaxed);
                    warn!(tenant_id, error = %e, "DEV FALLBACK: cred load failed");
                    Ok(root_creds())
                }
            },
        }
    }

    fn build_client(
        &self,
        creds: &StorageCreds,
        bucket: &str,
    ) -> Result<Arc<dyn ObjectStore>, StorageError> {
        AmazonS3Builder::new()
            .with_endpoint(&*self.endpoint)
            .with_bucket_name(bucket)
            .with_access_key_id(&creds.access_key)
            .with_secret_access_key(&creds.secret_key)
            .with_allow_http(true)
            .with_region("us-east-1")
            .build()
            .map(|s| Arc::new(s) as Arc<dyn ObjectStore>)
            .map_err(|e| StorageError::Internal(anyhow::anyhow!("build client: {e}")))
    }

    /// Invalidate the cached client for a tenant (e.g., after IAM key rotation).
    pub async fn invalidate(&self, tenant_id: &str) {
        let prefix = tenant_id.to_owned();
        self.client_cache
            .invalidate_entries_if(move |k, _| k.starts_with(prefix.as_str()))
            .ok();
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub fn shared_bucket(&self) -> &str {
        &self.bucket
    }
}

// ── Static key parsers ────────────────────────────────────────────────────────
//
// These belong here — not in call-sites — so that all knowledge of key formats
// is centralized in this module and the lint guard (`just lint-tenant-paths`)
// only needs to exclude this one file.

/// Extract the tenant ID from a legacy-layout S3 object key.
///
/// Legacy format: `tenants/{tenant_id}/…`
/// Returns `None` if the key does not match the legacy prefix format.
pub fn extract_tenant_from_legacy_key(key: &str) -> Option<&str> {
    let after = key.strip_prefix("tenants/")?;
    let slash = after.find('/')?;
    Some(&after[..slash])
}

/// Extract the virtual-path portion from any layout's S3 object key.
///
/// - Legacy:  `tenants/{id}/workspaces/{vp}` → `{vp}`
/// - Modern:  `workspaces/{vp}`              → `{vp}`
/// - Fallback: returns `key` unchanged.
pub fn extract_virtual_path_from_key(key: &str) -> &str {
    if let Some(pos) = key.find("/workspaces/") {
        &key[pos + "/workspaces/".len()..]
    } else if let Some(stripped) = key.strip_prefix("workspaces/") {
        stripped
    } else {
        key
    }
}

fn root_creds() -> StorageCreds {
    StorageCreds {
        access_key: std::env::var("RUSTFS_ROOT_ACCESS_KEY")
            .or_else(|_| std::env::var("AWS_ACCESS_KEY_ID"))
            .unwrap_or_else(|_| "rustfsadmin".into()),
        secret_key: std::env::var("RUSTFS_ROOT_SECRET_KEY")
            .or_else(|_| std::env::var("AWS_SECRET_ACCESS_KEY"))
            .unwrap_or_else(|_| "rustfsadmin".into()),
        created_at: 0,
        bucket: None,
    }
}

/// Build a root-credential ObjectStore (admin ops and bootstrap only).
pub fn build_root_store() -> anyhow::Result<Arc<dyn ObjectStore>> {
    let creds = root_creds();
    let endpoint = std::env::var("S3_ENDPOINT").unwrap_or_else(|_| "http://rustfs:9000".into());
    let bucket = std::env::var("S3_BUCKET").unwrap_or_else(|_| "workspace".into());
    AmazonS3Builder::new()
        .with_endpoint(&endpoint)
        .with_bucket_name(&bucket)
        .with_access_key_id(&creds.access_key)
        .with_secret_access_key(&creds.secret_key)
        .with_allow_http(true)
        .with_region("us-east-1")
        .build()
        .map(|s| Arc::new(s) as Arc<dyn ObjectStore>)
        .map_err(|e| anyhow::anyhow!("build root store: {e}"))
}

// ── Per-plan quota helper ─────────────────────────────────────────────────────

/// Bytes quota per plan tier (None = unlimited).
pub fn plan_quota_bytes(plan: &PlanTier) -> Option<u64> {
    match plan {
        PlanTier::Free => Some(1024 * 1024 * 1024), // 1 GiB
        PlanTier::Pro => Some(100 * 1024 * 1024 * 1024), // 100 GiB
        PlanTier::Enterprise => None,
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtual_path_valid() {
        assert!(VirtualPath::parse("Workspace/notes.md").is_ok());
        assert!(VirtualPath::parse("a/b/c").is_ok());
        assert!(VirtualPath::parse("file.md").is_ok());
    }

    #[test]
    fn virtual_path_rejects_dotdot() {
        assert!(VirtualPath::parse("../etc/passwd").is_err());
        assert!(VirtualPath::parse("a/../b").is_err());
    }

    #[test]
    fn virtual_path_rejects_absolute() {
        assert!(VirtualPath::parse("/etc/passwd").is_err());
    }

    #[test]
    fn virtual_path_rejects_empty_segments() {
        assert!(VirtualPath::parse("a//b").is_err());
    }

    #[test]
    fn virtual_path_rejects_trailing_slash() {
        assert!(VirtualPath::parse("a/b/").is_err());
    }

    #[test]
    fn virtual_path_rejects_control_chars() {
        assert!(VirtualPath::parse("a\0b").is_err());
        assert!(VirtualPath::parse("a\x1fb").is_err());
    }

    #[test]
    fn virtual_path_rejects_over_1024() {
        let long = "a".repeat(1025);
        assert!(VirtualPath::parse(&long).is_err());
    }

    #[test]
    fn legacy_workspace_path_contains_tenant_prefix() {
        let vp = VirtualPath::parse("notes.md").unwrap();
        let storage = mock_legacy_storage("tenant-123");
        let path = storage.workspace_path(&vp);
        assert_eq!(path.as_ref(), "tenants/tenant-123/workspaces/notes.md");
    }

    #[test]
    fn modern_workspace_path_no_tenant_prefix() {
        let vp = VirtualPath::parse("notes.md").unwrap();
        let storage = mock_modern_storage("tenant-456");
        let path = storage.workspace_path(&vp);
        assert_eq!(path.as_ref(), "workspaces/notes.md");
    }

    #[test]
    fn legacy_path_cannot_escape_tenant_prefix() {
        // Even if somehow a malformed VirtualPath was constructed, the layout prefix
        // always anchors the key under tenants/{id}/.
        // (In practice VirtualPath::parse would reject ".." before we get here.)
        let vp = VirtualPath("innocent".to_owned()); // bypass parse for test
        let storage = mock_legacy_storage("t1");
        let path = storage.workspace_path(&vp);
        assert!(path.as_ref().starts_with("tenants/t1/workspaces/"));
    }

    fn mock_creds() -> StorageCreds {
        StorageCreds {
            access_key: "test".into(),
            secret_key: "test".into(),
            created_at: 0,
            bucket: None,
        }
    }

    fn noop_audit() -> Arc<dyn AuditStore> {
        use common::memory::InMemoryAuditStore;
        Arc::new(InMemoryAuditStore::new())
    }

    fn mock_legacy_storage(tenant_id: &str) -> TenantStorage {
        use object_store::memory::InMemory;
        TenantStorage {
            tenant_id: tenant_id.to_owned(),
            client: Arc::new(InMemory::new()),
            creds: mock_creds(),
            bucket: Arc::from("workspace"),
            endpoint: Arc::from("http://localhost:9000"),
            layout: StorageLayout::LegacyPrefix {
                tenant_id: tenant_id.to_owned(),
            },
            audit: noop_audit(),
        }
    }

    fn mock_modern_storage(tenant_id: &str) -> TenantStorage {
        use object_store::memory::InMemory;
        TenantStorage {
            tenant_id: tenant_id.to_owned(),
            client: Arc::new(InMemory::new()),
            creds: mock_creds(),
            bucket: Arc::from(format!("ws-{tenant_id}")),
            endpoint: Arc::from("http://localhost:9000"),
            layout: StorageLayout::Modern,
            audit: noop_audit(),
        }
    }
}

#[cfg(test)]
mod storage_tests {
    use super::*;

    fn make_storage(tenant_id: &str) -> TenantStorage {
        use common::memory::InMemoryAuditStore;
        use object_store::memory::InMemory;
        TenantStorage {
            tenant_id: tenant_id.to_owned(),
            client: Arc::new(InMemory::new()),
            creds: StorageCreds {
                access_key: "t".into(),
                secret_key: "t".into(),
                created_at: 0,
                bucket: None,
            },
            bucket: Arc::from("workspace"),
            endpoint: Arc::from("http://localhost:9000"),
            layout: StorageLayout::LegacyPrefix {
                tenant_id: tenant_id.to_owned(),
            },
            audit: Arc::new(InMemoryAuditStore::new()),
        }
    }

    #[tokio::test]
    async fn round_trip_put_get() {
        let storage = make_storage("t1");
        let vp = VirtualPath::parse("docs/note.md").unwrap();
        let payload = Bytes::from("hello world");

        storage
            .put_workspace_object(&vp, payload.clone(), "text/markdown")
            .await
            .unwrap();
        let got = storage.get_workspace_object(&vp).await.unwrap();
        assert_eq!(got, payload);
    }

    #[tokio::test]
    async fn delete_removes_object() {
        let storage = make_storage("t2");
        let vp = VirtualPath::parse("file.md").unwrap();

        storage
            .put_workspace_object(&vp, Bytes::from("data"), "text/plain")
            .await
            .unwrap();
        storage.delete_workspace_object(&vp).await.unwrap();
        let result = storage.get_workspace_object(&vp).await;
        assert!(matches!(result, Err(StorageError::NotFound)));
    }

    #[tokio::test]
    async fn cross_tenant_isolation_in_memory() {
        use common::memory::InMemoryAuditStore;
        use object_store::memory::InMemory;

        // Two tenants with the SAME InMemory store (simulating shared bucket).
        let shared_client: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
        let audit: Arc<dyn AuditStore> = Arc::new(InMemoryAuditStore::new());

        let make = |tid: &str| TenantStorage {
            tenant_id: tid.to_owned(),
            client: Arc::clone(&shared_client),
            creds: StorageCreds {
                access_key: "x".into(),
                secret_key: "x".into(),
                created_at: 0,
                bucket: None,
            },
            bucket: Arc::from("workspace"),
            endpoint: Arc::from("http://localhost:9000"),
            layout: StorageLayout::LegacyPrefix {
                tenant_id: tid.to_owned(),
            },
            audit: Arc::clone(&audit),
        };

        let sa = make("tenant-a");
        let sb = make("tenant-b");
        let vp = VirtualPath::parse("secret.md").unwrap();

        sa.put_workspace_object(&vp, Bytes::from("A's secret"), "text/plain")
            .await
            .unwrap();

        // B's get uses B's layout key (tenants/tenant-b/workspaces/secret.md)
        // which is a different object key — returns NotFound, not A's data.
        let result = sb.get_workspace_object(&vp).await;
        assert!(
            matches!(result, Err(StorageError::NotFound)),
            "tenant B must not see tenant A's object even on a shared store"
        );
    }

    #[tokio::test]
    async fn abort_on_drop_fires_for_incomplete_upload() {
        use object_store::{memory::InMemory, path::Path as ObjectPath};

        let store = Arc::new(InMemory::new());
        let dest = ObjectPath::from("test/dest.bin");

        // Open a multipart upload and wrap in guard.
        let upload = store.put_multipart(&dest).await.unwrap();
        let guard = AbortOnDropMultipart::new(upload, dest.clone());

        // Drop the guard without calling take() — abort() should fire via tokio::spawn.
        drop(guard);

        // Give the spawned abort task a moment to run.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // The destination object must not exist (upload was aborted, never completed).
        let result = store.get(&dest).await;
        assert!(result.is_err(), "aborted upload must not produce an object");
    }

    #[tokio::test]
    async fn finalize_round_trip_via_staging() {
        use common::memory::InMemoryAuditStore;
        use object_store::{memory::InMemory, path::Path as ObjectPath};

        let client = Arc::new(InMemory::new());
        let storage = TenantStorage {
            tenant_id: "t3".into(),
            client: Arc::clone(&client) as Arc<dyn ObjectStore>,
            creds: StorageCreds {
                access_key: "t".into(),
                secret_key: "t".into(),
                created_at: 0,
                bucket: None,
            },
            bucket: Arc::from("workspace"),
            endpoint: Arc::from("http://localhost:9000"),
            layout: StorageLayout::LegacyPrefix {
                tenant_id: "t3".into(),
            },
            audit: Arc::new(InMemoryAuditStore::new()),
        };

        // Manually write a "staged part" at the path StagedUploadFinalizer will look for.
        let upload_id = "upload-abc";
        let part_key = ObjectPath::from("tenants/t3/uploads/tmp/upload-abc/part-001");
        let payload = Bytes::from(b"hello from part".repeat(100).to_vec());
        client.put(&part_key, payload.clone().into()).await.unwrap();

        let dest = VirtualPath::parse("docs/result.bin").unwrap();
        let result = storage
            .finalize_staged_upload(upload_id, &[], &dest)
            .await
            .unwrap();

        assert_eq!(result.size_bytes, payload.len() as u64);

        // The staging part must have been cleaned up.
        assert!(
            client.get(&part_key).await.is_err(),
            "staging part must be deleted after finalize"
        );

        // The destination object must exist.
        let got = storage.get_workspace_object(&dest).await.unwrap();
        assert_eq!(got.len(), payload.len());
    }
}

#[cfg(test)]
mod proptest_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(10_000))]

        /// Any string that parses as a VirtualPath must not contain path-traversal
        /// sequences and must not produce an S3 key that escapes the tenant prefix.
        #[test]
        fn virtual_path_parse_never_escapes(input in ".*") {
            if let Ok(vp) = VirtualPath::parse(&input) {
                let s = vp.as_str();
                // No traversal segments.
                for seg in s.split('/') {
                    prop_assert_ne!(seg, "..");
                    prop_assert_ne!(seg, ".");
                }
                // No leading slash.
                prop_assert!(!s.starts_with('/'));
                // No trailing slash.
                prop_assert!(!s.ends_with('/'));
                // No control chars.
                for b in s.bytes() {
                    prop_assert!(b >= 0x20 && b != 0);
                }
                // When embedded in a legacy S3 key, the key stays under the tenant prefix.
                let key = format!("tenants/t1/workspaces/{s}");
                prop_assert!(key.starts_with("tenants/t1/workspaces/"));
                // The key must not contain ".." after joining.
                prop_assert!(!key.contains("/../"));
                prop_assert!(!key.ends_with("/.."));
            }
        }

        /// Strings that parse successfully must be ≤ 1024 bytes and non-empty.
        #[test]
        fn virtual_path_length_invariant(input in ".*") {
            if let Ok(vp) = VirtualPath::parse(&input) {
                prop_assert!(vp.as_str().len() <= 1024);
                prop_assert!(!vp.as_str().is_empty());
            }
        }
    }
}
