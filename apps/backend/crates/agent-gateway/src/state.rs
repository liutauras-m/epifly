use crate::mw::RateLimiter;
use agent_core::{
    ConversationService, DefaultConversationService, LlmRegistry, MinioWorkspaceContent,
    QdrantAuditStore, QdrantThreadStore, QdrantWorkspaceStore, CapabilityAdmin, ToolDiscovery,
    ToolRegistry, build_admin,
};
use agent_core::llm::providers::anthropic::AnthropicProvider;
use common::audit::AuditStore;
use common::memory::{
    InMemoryAuditStore, InMemoryThreadStore, InMemoryWorkspaceContent, InMemoryWorkspaceStore,
    ThreadStore, WorkspaceContentStore, WorkspaceStore,
};
use jobs::{JobAdmin, JobContext, JobExecutor, JobRegistry};
use jobs::jobs::{AuditLogCleanupJob, CapabilityHealthCheckJob, VideoTranscriptionJob};
use object_store::{ObjectStore, aws::AmazonS3Builder};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{info, warn};

pub struct AppState {
    pub registry: Arc<Mutex<ToolRegistry>>,
    pub rate_limiter: RateLimiter,
    /// LLM provider registry — single source of truth for all model access.
    pub llm: Arc<LlmRegistry>,
    /// MinIO / S3-compatible file store (None if not configured)
    pub file_store: Option<Arc<dyn ObjectStore>>,
    /// In-memory map of download tokens → (object_key, issued_at, ttl, tenant_id)
    pub presigned_tokens: Mutex<HashMap<String, (String, std::time::Instant, std::time::Duration, String)>>,
    /// Persistent conversation memory backed by Qdrant (or in-memory in test mode)
    pub thread_store: Arc<dyn ThreadStore>,
    /// Append-only audit log backed by Qdrant (or in-memory in test mode)
    pub audit_store: Arc<dyn AuditStore>,
    /// Workspace node index — Qdrant (or in-memory in test mode)
    pub workspace_store: Arc<dyn WorkspaceStore>,
    /// Workspace markdown body store — MinIO (or in-memory in test mode)
    pub workspace_content: Arc<dyn WorkspaceContentStore>,
    /// Unified conversation service (single source of truth for thread lifecycle).
    pub conversation_service: Arc<dyn ConversationService>,
    /// Super-admin capability management service.
    pub tool_admin: Arc<CapabilityAdmin>,
    /// Scheduled + background job registry.
    pub job_registry: Arc<JobRegistry>,
    /// Background task executor (in-memory).
    pub job_executor: Arc<JobExecutor>,
    /// Super-admin job management facade.
    pub job_admin: Arc<JobAdmin>,
}

impl AppState {
    pub fn from_env() -> common::error::Result<Self> {
        if std::env::var("CONUSAI_TEST_MODE").as_deref() == Ok("1") {
            return Self::with_in_memory_stores();
        }

        let llm = Arc::new(build_llm_registry());
        let mut registry = ToolRegistry::with_default_factories(Arc::clone(&llm));
        ToolDiscovery::from_env().discover_into(&mut registry)?;

        let qdrant_url =
            std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6333".into());

        let file_store = init_file_store();
        let thread_store: Arc<dyn ThreadStore> = Arc::new(QdrantThreadStore::new(&qdrant_url));
        let audit_store: Arc<dyn AuditStore> = Arc::new(QdrantAuditStore::new(&qdrant_url));
        let workspace_store: Arc<dyn WorkspaceStore> =
            Arc::new(QdrantWorkspaceStore::new(&qdrant_url));

        let workspace_content: Arc<dyn WorkspaceContentStore> = match &file_store {
            Some(fs) => Arc::new(MinioWorkspaceContent::new(Arc::clone(fs))),
            None => {
                warn!("file store not configured — workspace content (MinIO) will be unavailable");
                Arc::new(NoopWorkspaceContent)
            }
        };

        let conversation_service: Arc<dyn ConversationService> =
            Arc::new(DefaultConversationService {
                thread_store: Arc::clone(&thread_store),
                workspace_store: Arc::clone(&workspace_store),
            });

        let registry = Arc::new(Mutex::new(registry));
        let tool_admin = Arc::new(build_admin(Arc::clone(&registry), Arc::clone(&audit_store)));

        // Build job infrastructure
        let minio_endpoint = std::env::var("MINIO_ENDPOINT")
            .or_else(|_| std::env::var("S3_ENDPOINT"))
            .ok();
        let bucket = std::env::var("MINIO_BUCKET")
            .unwrap_or_else(|_| "conusai".into());
        let job_ctx = Arc::new(JobContext::new(
            Arc::clone(&audit_store),
            &qdrant_url,
            minio_endpoint,
            Some(bucket),
        ));
        let job_registry = build_job_registry(job_ctx);
        let job_executor = JobExecutor::new(Arc::clone(&job_registry));
        let job_admin = Arc::new(JobAdmin::new(Arc::clone(&job_registry), Arc::clone(&job_executor)));

        Ok(Self {
            registry,
            rate_limiter: RateLimiter::new(),
            llm,
            file_store,
            presigned_tokens: Mutex::new(HashMap::new()),
            thread_store,
            audit_store,
            workspace_store,
            workspace_content,
            conversation_service,
            tool_admin,
            job_registry,
            job_executor,
            job_admin,
        })
    }

    /// Build an `AppState` backed entirely by in-memory stores — no Qdrant or MinIO required.
    ///
    /// Activated when `CONUSAI_TEST_MODE=1` is set in the environment.  All data is lost on
    /// process exit.  Intended for integration tests and CI pipelines without Docker.
    pub fn with_in_memory_stores() -> common::error::Result<Self> {
        info!("CONUSAI_TEST_MODE=1 — using in-memory stores (no Qdrant / MinIO)");

        let llm = Arc::new(build_llm_registry());
        let mut registry = ToolRegistry::with_default_factories(Arc::clone(&llm));
        ToolDiscovery::from_env().discover_into(&mut registry)?;

        let thread_store: Arc<dyn ThreadStore> = Arc::new(InMemoryThreadStore::new());
        let workspace_store: Arc<dyn WorkspaceStore> = Arc::new(InMemoryWorkspaceStore::new());
        let conversation_service: Arc<dyn ConversationService> =
            Arc::new(DefaultConversationService {
                thread_store: Arc::clone(&thread_store),
                workspace_store: Arc::clone(&workspace_store),
            });

        let audit_store: Arc<dyn AuditStore> = Arc::new(InMemoryAuditStore::new());
        let registry = Arc::new(Mutex::new(registry));
        let tool_admin = Arc::new(build_admin(Arc::clone(&registry), Arc::clone(&audit_store)));

        // Build job infrastructure (no Qdrant/MinIO in test mode)
        let job_ctx = Arc::new(JobContext::new(
            Arc::clone(&audit_store),
            "http://localhost:6333",
            None,
            None,
        ));
        let job_registry = build_job_registry(job_ctx);
        let job_executor = JobExecutor::new(Arc::clone(&job_registry));
        let job_admin = Arc::new(JobAdmin::new(Arc::clone(&job_registry), Arc::clone(&job_executor)));

        Ok(Self {
            registry,
            rate_limiter: RateLimiter::new(),
            llm,
            file_store: None,
            presigned_tokens: Mutex::new(HashMap::new()),
            thread_store,
            audit_store,
            workspace_store,
            workspace_content: Arc::new(InMemoryWorkspaceContent::new()),
            conversation_service,
            tool_admin,
            job_registry,
            job_executor,
            job_admin,
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

/// Build an `LlmRegistry` from the environment.
///
/// If `ANTHROPIC_API_KEY` is absent the registry still starts with no providers;
/// routes that call `.complete()` will return an appropriate error at request
/// time rather than crashing at startup.
fn build_llm_registry() -> LlmRegistry {
    use agent_core::llm::types::LlmBinding;
    use std::collections::HashMap;

    let mut providers: HashMap<String, Arc<dyn agent_core::CompletionProvider>> = HashMap::new();

    match AnthropicProvider::from_env() {
        Ok(p) => {
            providers.insert("anthropic".into(), Arc::new(p));
        }
        Err(e) => {
            tracing::warn!(error = %e, "Anthropic provider unavailable at startup");
        }
    }

    // Default binding: use anthropic/haiku as the fallback.
    let default_binding = LlmBinding {
        provider: "anthropic".into(),
        model: "claude-haiku-4-5".into(),
    };
    let aliases = HashMap::new();
    LlmRegistry::new(providers, aliases, default_binding)
}

/// Build a `JobRegistry` pre-populated with the platform's built-in jobs.
fn build_job_registry(ctx: Arc<JobContext>) -> Arc<JobRegistry> {
    let mut registry = JobRegistry::new(ctx);
    registry.register_scheduled(CapabilityHealthCheckJob);
    registry.register_scheduled(AuditLogCleanupJob);
    registry.register_background(VideoTranscriptionJob);
    Arc::new(registry)
}
