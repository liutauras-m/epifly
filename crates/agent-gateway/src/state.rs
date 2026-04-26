use crate::mw::RateLimiter;
use agent_core::{CapabilityDiscovery, CapabilityRegistry, QdrantThreadStore};
use common::memory::ThreadStore;
use object_store::{aws::AmazonS3Builder, ObjectStore};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{info, warn};

pub struct AppState {
    pub registry: Mutex<CapabilityRegistry>,
    pub rate_limiter: RateLimiter,
    /// MinIO / S3-compatible file store (None if not configured)
    pub file_store: Option<Arc<dyn ObjectStore>>,
    /// Qdrant REST base URL (also used by thread store)
    pub qdrant_url: String,
    /// In-memory map of download tokens → (object_key, issued_at, ttl)
    pub presigned_tokens: Mutex<HashMap<String, (String, std::time::Instant, std::time::Duration)>>,
    /// Persistent conversation memory backed by Qdrant
    pub thread_store: Arc<dyn ThreadStore>,
}

impl AppState {
    pub fn from_env() -> common::error::Result<Self> {
        let discovery = CapabilityDiscovery::from_env();
        let registry = discovery.discover()?;

        let qdrant_url =
            std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6333".into());

        let file_store = init_file_store();
        let thread_store = Arc::new(QdrantThreadStore::new(&qdrant_url));

        Ok(Self {
            registry: Mutex::new(registry),
            rate_limiter: RateLimiter::new(),
            file_store,
            qdrant_url,
            presigned_tokens: Mutex::new(HashMap::new()),
            thread_store,
        })
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
