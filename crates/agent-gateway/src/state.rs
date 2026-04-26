use crate::mw::RateLimiter;
use agent_core::{
    CapabilityDiscovery, CapabilityRegistry, MinioWorkspaceContent, QdrantAuditStore,
    QdrantThreadStore, QdrantWorkspaceStore, native_capability_card,
};
use common::audit::AuditStore;
use common::memory::{ThreadStore, WorkspaceContentStore, WorkspaceStore};
use object_store::{ObjectStore, aws::AmazonS3Builder};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{info, warn};

pub struct AppState {
    pub registry: Mutex<CapabilityRegistry>,
    pub rate_limiter: RateLimiter,
    /// MinIO / S3-compatible file store (None if not configured)
    pub file_store: Option<Arc<dyn ObjectStore>>,
    /// Qdrant REST base URL (also used by thread store)
    #[allow(dead_code)]
    pub qdrant_url: String,
    /// In-memory map of download tokens → (object_key, issued_at, ttl)
    pub presigned_tokens: Mutex<HashMap<String, (String, std::time::Instant, std::time::Duration)>>,
    /// Persistent conversation memory backed by Qdrant
    pub thread_store: Arc<dyn ThreadStore>,
    /// Append-only audit log backed by Qdrant
    pub audit_store: Arc<dyn AuditStore>,
    /// Workspace node index (Qdrant)
    pub workspace_store: Arc<dyn WorkspaceStore>,
    /// Workspace markdown body store (MinIO)
    pub workspace_content: Arc<dyn WorkspaceContentStore>,
}

impl AppState {
    pub fn from_env() -> common::error::Result<Self> {
        let discovery = CapabilityDiscovery::from_env();
        let mut registry = discovery.discover()?;
        registry.register(native_capability_card());

        let qdrant_url =
            std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6333".into());

        let file_store = init_file_store();
        let thread_store = Arc::new(QdrantThreadStore::new(&qdrant_url));
        let audit_store = Arc::new(QdrantAuditStore::new(&qdrant_url));
        let workspace_store = Arc::new(QdrantWorkspaceStore::new(&qdrant_url));

        let workspace_content: Arc<dyn WorkspaceContentStore> = match &file_store {
            Some(fs) => Arc::new(MinioWorkspaceContent::new(Arc::clone(fs))),
            None => {
                warn!("file store not configured — workspace content (MinIO) will be unavailable");
                Arc::new(NoopWorkspaceContent)
            }
        };

        Ok(Self {
            registry: Mutex::new(registry),
            rate_limiter: RateLimiter::new(),
            file_store,
            qdrant_url,
            presigned_tokens: Mutex::new(HashMap::new()),
            thread_store,
            audit_store,
            workspace_store,
            workspace_content,
        })
    }
}

/// Fallback content store used when MinIO is not configured.
struct NoopWorkspaceContent;

#[async_trait::async_trait]
impl WorkspaceContentStore for NoopWorkspaceContent {
    async fn read(&self, _: &str, _: &str) -> anyhow::Result<String> {
        anyhow::bail!("workspace content store not configured (MINIO_ENDPOINT missing)")
    }
    async fn write(&self, _: &str, _: &str, _: &str) -> anyhow::Result<()> {
        anyhow::bail!("workspace content store not configured (MINIO_ENDPOINT missing)")
    }
    async fn delete(&self, _: &str, _: &str) -> anyhow::Result<()> {
        anyhow::bail!("workspace content store not configured (MINIO_ENDPOINT missing)")
    }
}

fn init_file_store() -> Option<Arc<dyn ObjectStore>> {
    let endpoint = std::env::var("MINIO_ENDPOINT")
        .or_else(|_| std::env::var("S3_ENDPOINT"))
        .unwrap_or_else(|_| "http://localhost:9000".into());

    let bucket = std::env::var("MINIO_BUCKET")
        .or_else(|_| std::env::var("S3_BUCKET"))
        .unwrap_or_else(|_| "conusai".into());

    let access_key = std::env::var("MINIO_ACCESS_KEY")
        .or_else(|_| std::env::var("AWS_ACCESS_KEY_ID"))
        .unwrap_or_else(|_| "minioadmin".into());

    let secret_key = std::env::var("MINIO_SECRET_KEY")
        .or_else(|_| std::env::var("AWS_SECRET_ACCESS_KEY"))
        .unwrap_or_else(|_| "minioadmin".into());

    match AmazonS3Builder::new()
        .with_endpoint(&endpoint)
        .with_bucket_name(&bucket)
        .with_access_key_id(&access_key)
        .with_secret_access_key(&secret_key)
        .with_allow_http(true)
        .with_region("us-east-1")
        .build()
    {
        Ok(store) => {
            info!(endpoint, bucket, "MinIO/S3 object store initialised");
            Some(Arc::new(store))
        }
        Err(e) => {
            warn!(
                error = %e,
                "Failed to initialise file store; file upload endpoints will be unavailable"
            );
            None
        }
    }
}
