use crate::mw::{RateLimiter, RouterQuotaConfig};
use crate::routes::admin_devices::DeviceToken;
use agent_core::llm::providers::anthropic::AnthropicProvider;
use agent_core::{
    ArtifactBridge, BulkCapabilityFactory, CapabilityAdmin, CapabilityDiscovery,
    CapabilityRegistry, CapabilitySpecFactory, ConversationService, CredentialStore,
    DefaultConversationService, EmbeddingService, LlmRegistry, NamespaceFilter,
    NoopEmbeddingService, OpenAiEmbeddingService, QdrantVectorStore, RealtimeService,
    RedbMetadataStore, RustFsContentStore, SemanticCapabilityRouter, SemanticRouterConfig,
    StorageQuotaService, TenantOnboardingService, TenantStorageFactory, build_admin,
    build_root_store,
};
use agent_core::identity::IdentityManager;
use agent_core::{LegacyIdentityProvider, ZitadelProvider};
use billing_core::{BillingProvider, LagoProvider, PlanCatalog, QuotaChecker};
use std::sync::Arc as StdArc;
use common::audit::AuditStore;
use common::memory::{
    InMemoryAuditStore, InMemoryThreadStore, InMemoryWorkspaceContent, InMemoryWorkspaceStore,
    ThreadStore, WorkspaceContentStore, WorkspaceStore,
};
use jobs::jobs::{AuditLogCleanupJob, CapabilityHealthCheckJob, LagoReconcileJob, RustFsKeyRotationJob, TenantBucketMigrationJob, VideoTranscriptionJob};
use jobs::{JobAdmin, JobContext, JobExecutor, JobRegistry};
use object_store::aws::AmazonS3Builder;
use object_store::ObjectStore;
use redb::Database;
use rustfs_admin::RustFsAdminClient;
use crate::metrics::RustFsMetrics;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{info, warn};

pub struct AppState {
    pub registry: Arc<Mutex<CapabilityRegistry>>,
    pub rate_limiter: RateLimiter,
    pub llm: Arc<LlmRegistry>,
    /// RustFS / S3-compatible file store (root credentials — admin path only)
    pub file_store: Option<Arc<dyn ObjectStore>>,
    /// RustFS admin client (root credentials — used for bootstrap and provisioning)
    pub rustfs_admin: Option<Arc<RustFsAdminClient>>,
    /// Per-tenant credential store (encrypted in redb)
    pub cred_store: Option<Arc<CredentialStore>>,
    /// Per-tenant storage factory — single source of truth for all S3 operations
    pub tenant_storage: Option<Arc<TenantStorageFactory>>,
    /// Tenant onboarding service — provisions IAM + default workspace root
    pub onboarding: Option<Arc<TenantOnboardingService>>,
    /// Per-tenant storage quota service
    pub storage_quota: Arc<StorageQuotaService>,
    /// Prometheus metrics for storage operations and fallbacks.
    pub rustfs_metrics: Option<Arc<RustFsMetrics>>,
    /// Per-tenant single-flight mutex for the onboarding safety net in the `tree` route.
    pub onboarding_guards: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>>,
    /// In-memory device tokens for browser-shell (keyed by blake3 hash of plaintext token).
    pub device_tokens: Mutex<HashMap<String, DeviceToken>>,
    pub thread_store: Arc<dyn ThreadStore>,
    pub audit_store: Arc<dyn AuditStore>,
    pub workspace_store: Arc<dyn WorkspaceStore>,
    pub workspace_content: Arc<dyn WorkspaceContentStore>,
    pub conversation_service: Arc<dyn ConversationService>,
    pub tool_admin: Arc<CapabilityAdmin>,
    pub job_registry: Arc<JobRegistry>,
    pub job_executor: Arc<JobExecutor>,
    pub job_admin: Arc<JobAdmin>,
    pub embedding_service: Arc<dyn EmbeddingService>,
    pub vector_store: Arc<QdrantVectorStore>,
    pub realtime_service: Arc<RealtimeService>,
    pub semantic_router: Arc<SemanticCapabilityRouter>,
    pub router_quota: RouterQuotaConfig,
    pub capability_spec_factory: Option<Arc<CapabilitySpecFactory>>,
    pub artifact_bridge: Option<Arc<ArtifactBridge>>,
    /// Identity provider (legacy HMAC/JWT or Zitadel OIDC).
    pub identity: StdArc<dyn IdentityManager>,
    /// Billing provider (Lago). None when LAGO_API_KEY not configured.
    pub billing: Option<StdArc<dyn BillingProvider>>,
    /// In-process quota checker.
    pub quota: Option<StdArc<QuotaChecker>>,
    /// Plan catalog loaded at boot.
    pub plan_catalog: StdArc<PlanCatalog>,
}

impl AppState {
    pub async fn from_env() -> common::error::Result<Self> {
        if std::env::var("CONUSAI_TEST_MODE").as_deref() == Ok("1") {
            return Self::with_in_memory_stores();
        }

        if std::env::var_os("PLATFORM_ADMIN_TOKEN").is_none_or(|v| v.is_empty())
            && !cfg!(debug_assertions)
        {
            tracing::warn!(
                config = "missing",
                env = "PLATFORM_ADMIN_TOKEN",
                "/admin/capabilities/register is OPEN in a non-debug build"
            );
        }

        let llm = Arc::new(build_llm_registry());

        // ── Persistent stores (redb + Qdrant + RustFS) ───────────────────────

        let redb_path = std::env::var("REDB_PATH")
            .unwrap_or_else(|_| "/data/conusai.redb".into());
        let metadata_store: Arc<RedbMetadataStore> = RedbMetadataStore::open(&redb_path)
            .map_err(|e| common::error::ConusAiError::Storage(e.to_string()))?;

        // Open a separate Database handle for the credential store.
        let cred_db: Arc<Database> = Arc::new(
            Database::create(&redb_path)
                .map_err(|e| common::error::ConusAiError::Storage(e.to_string()))?,
        );
        let cred_store: Arc<CredentialStore> = Arc::new(
            CredentialStore::new(cred_db)
                .map_err(|e| common::error::ConusAiError::Storage(e.to_string()))?,
        );

        let qdrant_url = std::env::var("QDRANT_URL")
            .unwrap_or_else(|_| "http://qdrant:6334".into());
        let vector_store = Arc::new(
            QdrantVectorStore::connect(&qdrant_url)
                .await
                .map_err(|e| common::error::ConusAiError::Storage(e.to_string()))?,
        );

        let thread_store: Arc<dyn ThreadStore> = {
            let s: Arc<RedbMetadataStore> = Arc::clone(&metadata_store);
            s
        };
        let audit_store: Arc<dyn AuditStore> = {
            let s: Arc<RedbMetadataStore> = Arc::clone(&metadata_store);
            s
        };
        let workspace_store: Arc<dyn WorkspaceStore> = {
            let s: Arc<RedbMetadataStore> = Arc::clone(&metadata_store);
            s
        };

        let tenant_storage = Arc::new(TenantStorageFactory::new(
            Arc::clone(&cred_store),
            Arc::clone(&audit_store),
        ));

        let workspace_content: Arc<dyn WorkspaceContentStore> =
            RustFsContentStore::new(Arc::clone(&tenant_storage));

        info!("workspace content: RustFS/S3 object store (per-tenant IAM via TenantStorageFactory)");

        let file_store = init_file_store();

        let rustfs_admin = init_rustfs_admin();

        let onboarding = rustfs_admin.as_ref().map(|admin| {
            TenantOnboardingService::new(
                Arc::clone(&workspace_store),
                Arc::clone(&tenant_storage),
                Arc::clone(&cred_store),
                Arc::clone(admin),
            )
        });

        let storage_quota = StorageQuotaService::new(Arc::clone(&tenant_storage));

        let embedding_service: Arc<dyn EmbeddingService> = match std::env::var("EMBEDDING_BACKEND")
            .as_deref()
        {
            Ok("local") => {
                #[cfg(feature = "local-embeddings")]
                {
                    info!("embedding service: local fastembed");
                    Arc::new(agent_core::LocalEmbeddingService::from_env()?)
                }
                #[cfg(not(feature = "local-embeddings"))]
                {
                    warn!(
                        "EMBEDDING_BACKEND=local but feature local-embeddings not compiled — falling back to noop"
                    );
                    Arc::new(NoopEmbeddingService)
                }
            }
            Ok("openai") | Err(_) => match OpenAiEmbeddingService::from_env() {
                Ok(svc) => {
                    info!("embedding service: OpenAI text-embedding-3-small");
                    Arc::new(svc)
                }
                Err(e) => {
                    warn!(error = %e, "embedding service unavailable — vector search disabled");
                    Arc::new(NoopEmbeddingService)
                }
            },
            Ok(other) => {
                return Err(common::error::ConusAiError::Config(format!(
                    "unknown EMBEDDING_BACKEND={other}"
                )));
            }
        };

        let mut registry_raw = CapabilityRegistry::with_all_factories(Arc::clone(&llm));
        CapabilityDiscovery::from_env().discover_into(&mut registry_raw)?;

        let capability_spec_factory = Arc::new(CapabilitySpecFactory::new(
            Arc::clone(&llm),
            Arc::clone(&embedding_service),
            Arc::clone(&vector_store),
        ));
        match capability_spec_factory.load_batch(&mut registry_raw).await {
            Ok(loaded) => info!(loaded, "capability-spec bulk load complete"),
            Err(e) => warn!(error = %e, "capability-spec bulk load failed; continuing startup"),
        }

        // Register the workspace built-in capability before building the registry Arc.
        {
            use crate::capabilities::workspace::WorkspaceProvider;
            let ws_card = WorkspaceProvider::new(
                Arc::clone(&workspace_store),
                Arc::clone(&workspace_content),
            )
            .into_card();
            registry_raw.register(ws_card);
        }

        let registry = Arc::new(Mutex::new(registry_raw));

        let router_cfg = SemanticRouterConfig {
            top_k: std::env::var("SEMANTIC_ROUTER_TOP_K")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(20),
            namespace: NamespaceFilter::Any,
            include_always: vec!["workspace".into()],
            ..Default::default()
        };
        let semantic_router = SemanticCapabilityRouter::new(
            Arc::clone(&registry),
            Arc::clone(&vector_store),
            Arc::clone(&embedding_service),
            router_cfg,
        );

        let conversation_service: Arc<dyn ConversationService> =
            Arc::new(DefaultConversationService {
                thread_store: Arc::clone(&thread_store),
                workspace_store: Arc::clone(&workspace_store),
            });

        let tool_admin = Arc::new(build_admin(Arc::clone(&registry), Arc::clone(&audit_store)));

        // ── Identity provider ─────────────────────────────────────────────
        let auth_provider = std::env::var("CONUSAI_AUTH_PROVIDER")
            .unwrap_or_else(|_| "legacy".into());
        let identity: StdArc<dyn IdentityManager> = if auth_provider == "zitadel" {
            match ZitadelProvider::from_env() {
                Ok(p) => {
                    info!("identity provider: Zitadel OIDC");
                    StdArc::new(p) as StdArc<dyn IdentityManager>
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Zitadel provider init failed — falling back to legacy");
                    StdArc::new(LegacyIdentityProvider::from_env()) as StdArc<dyn IdentityManager>
                }
            }
        } else {
            info!("identity provider: legacy HMAC/JWT");
            StdArc::new(LegacyIdentityProvider::from_env()) as StdArc<dyn IdentityManager>
        };

        // ── Plan catalog ──────────────────────────────────────────────────
        let plan_catalog = StdArc::new(PlanCatalog::load());
        info!(plans = plan_catalog.list().len(), "plan catalog loaded");

        // ── Billing (Lago) — initialized before job registry so reconcile job has it ──
        let (billing, quota): (Option<StdArc<dyn BillingProvider>>, Option<StdArc<QuotaChecker>>) =
            match LagoProvider::from_env() {
                Ok(provider) => {
                    info!("billing provider: Lago");
                    let quota_checker = StdArc::new(QuotaChecker::new(StdArc::clone(&plan_catalog)));
                    (Some(StdArc::new(provider) as StdArc<dyn BillingProvider>), Some(quota_checker))
                }
                Err(e) => {
                    warn!(error = %e, "Lago not configured — billing/metering disabled");
                    (None, None)
                }
            };

        let s3_endpoint = std::env::var("S3_ENDPOINT").ok();
        let bucket = std::env::var("S3_BUCKET")
            .unwrap_or_else(|_| "workspace".into());
        let mut job_ctx = JobContext::new(
            Arc::clone(&audit_store),
            s3_endpoint,
            Some(bucket),
        );
        if let (Some(ra), Some(cs)) = (rustfs_admin.as_ref(), Some(&cred_store)) {
            job_ctx = job_ctx.with_rustfs(Arc::clone(ra), Arc::clone(cs));
        }
        job_ctx = job_ctx.with_storage(
            Arc::clone(&tenant_storage),
            Arc::clone(&workspace_store),
        );
        let job_ctx = Arc::new(job_ctx);
        let job_registry = build_job_registry(job_ctx, billing.clone());
        let job_executor = JobExecutor::new(Arc::clone(&job_registry));
        let job_admin = Arc::new(JobAdmin::new(
            Arc::clone(&job_registry),
            Arc::clone(&job_executor),
        ));

        let artifact_bridge = file_store.as_ref().map(|fs| {
            ArtifactBridge::new(Arc::clone(fs), Arc::clone(&workspace_content))
        });

        let realtime_service = RealtimeService::new();

        Ok(Self {
            registry,
            rate_limiter: RateLimiter::new(),
            llm,
            file_store,
            rustfs_admin,
            cred_store: Some(cred_store),
            tenant_storage: Some(tenant_storage),
            onboarding,
            storage_quota,
            rustfs_metrics: None,
            onboarding_guards: Arc::new(Mutex::new(HashMap::new())),
            device_tokens: Mutex::new(HashMap::new()),
            thread_store,
            audit_store,
            workspace_store,
            workspace_content,
            conversation_service,
            tool_admin,
            job_registry,
            job_executor,
            job_admin,
            embedding_service,
            vector_store,
            realtime_service,
            semantic_router,
            router_quota: RouterQuotaConfig::from_env(),
            capability_spec_factory: Some(capability_spec_factory),
            artifact_bridge,
            identity,
            billing,
            quota,
            plan_catalog,
        })
    }

    /// Build an `AppState` backed entirely by in-memory stores — no external dependencies.
    ///
    /// Activated when `CONUSAI_TEST_MODE=1` is set.
    pub fn with_in_memory_stores() -> common::error::Result<Self> {
        info!("CONUSAI_TEST_MODE=1 — using in-memory stores (no Qdrant / RustFS)");

        let llm = Arc::new(build_llm_registry());
        let mut registry = CapabilityRegistry::with_default_factories(Arc::clone(&llm));
        CapabilityDiscovery::from_env().discover_into(&mut registry)?;

        let thread_store: Arc<dyn ThreadStore> = Arc::new(InMemoryThreadStore::new());
        let workspace_store: Arc<dyn WorkspaceStore> = Arc::new(InMemoryWorkspaceStore::new());
        let workspace_content: Arc<dyn WorkspaceContentStore> =
            Arc::new(InMemoryWorkspaceContent::new());
        let conversation_service: Arc<dyn ConversationService> =
            Arc::new(DefaultConversationService {
                thread_store: Arc::clone(&thread_store),
                workspace_store: Arc::clone(&workspace_store),
            });

        let audit_store: Arc<dyn AuditStore> = Arc::new(InMemoryAuditStore::new());

        // Register workspace capability before building the registry Arc.
        {
            use crate::capabilities::workspace::WorkspaceProvider;
            let ws_card = WorkspaceProvider::new(
                Arc::clone(&workspace_store),
                Arc::clone(&workspace_content),
            )
            .into_card();
            registry.register(ws_card);
        }

        let registry = Arc::new(Mutex::new(registry));
        let tool_admin = Arc::new(build_admin(Arc::clone(&registry), Arc::clone(&audit_store)));

        let embedding_service: Arc<dyn EmbeddingService> = Arc::new(NoopEmbeddingService);
        let vector_store = Arc::new(QdrantVectorStore::noop());
        let semantic_router = SemanticCapabilityRouter::new(
            Arc::clone(&registry),
            Arc::clone(&vector_store),
            Arc::clone(&embedding_service),
            SemanticRouterConfig {
                include_always: vec!["workspace".into()],
                ..Default::default()
            },
        );

        let job_ctx = Arc::new(JobContext::new(Arc::clone(&audit_store), None, None));
        let job_registry = build_job_registry(job_ctx, None);
        let job_executor = JobExecutor::new(Arc::clone(&job_registry));
        let job_admin = Arc::new(JobAdmin::new(
            Arc::clone(&job_registry),
            Arc::clone(&job_executor),
        ));

        let plan_catalog = StdArc::new(PlanCatalog::default());
        let identity: StdArc<dyn IdentityManager> =
            StdArc::new(LegacyIdentityProvider::from_env()) as StdArc<dyn IdentityManager>;

        Ok(Self {
            registry,
            rate_limiter: RateLimiter::new(),
            llm,
            file_store: None,
            rustfs_admin: None,
            cred_store: None,
            tenant_storage: None,
            onboarding: None,
            storage_quota: noop_quota_service(),
            rustfs_metrics: None,
            onboarding_guards: Arc::new(Mutex::new(HashMap::new())),
            device_tokens: Mutex::new(HashMap::new()),
            thread_store,
            audit_store,
            workspace_store,
            workspace_content,
            conversation_service,
            tool_admin,
            job_registry,
            job_executor,
            job_admin,
            embedding_service,
            vector_store,
            realtime_service: RealtimeService::new(),
            semantic_router,
            router_quota: RouterQuotaConfig::default(),
            capability_spec_factory: None,
            artifact_bridge: None,
            identity,
            billing: None,
            quota: None,
            plan_catalog,
        })
    }
}

/// Build a quota service backed by a dev-fallback factory (for test mode).
fn noop_quota_service() -> Arc<StorageQuotaService> {
    // In test mode there's no redb or RustFS. Use a factory that always falls back.
    // RUSTFS_DEV_FALLBACK_ROOT=on is set by the test harness so for_tenant won't error.
    // Quota checks are disabled in test mode anyway (RUSTFS_QUOTAS=off default).
    use agent_core::store::creds::CredentialStore;
    use redb::Database;
    let db = Arc::new(
        Database::builder()
            .create_with_backend(redb::backends::InMemoryBackend::new())
            .unwrap(),
    );
    let cs = Arc::new(CredentialStore::new(db).unwrap());
    let audit: Arc<dyn common::audit::AuditStore> =
        Arc::new(common::memory::InMemoryAuditStore::new());
    // Safety: single-threaded at startup in test/dev mode; no concurrent env reads.
    unsafe { std::env::set_var("RUSTFS_DEV_FALLBACK_ROOT", "on") };
    StorageQuotaService::new(Arc::new(TenantStorageFactory::new(cs, audit)))
}

fn init_file_store() -> Option<Arc<dyn ObjectStore>> {
    let endpoint = std::env::var("S3_ENDPOINT")
        .unwrap_or_else(|_| "http://rustfs:9000".into());
    let bucket = std::env::var("S3_BUCKET")
        .unwrap_or_else(|_| "workspace".into());
    let access_key = std::env::var("RUSTFS_ROOT_ACCESS_KEY")
        .or_else(|_| std::env::var("AWS_ACCESS_KEY_ID"))
        .unwrap_or_else(|_| "rustfsadmin".into());
    let secret_key = std::env::var("RUSTFS_ROOT_SECRET_KEY")
        .or_else(|_| std::env::var("AWS_SECRET_ACCESS_KEY"))
        .unwrap_or_else(|_| "rustfsadmin".into());

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
            info!(endpoint, bucket, "RustFS/S3 root file store initialised");
            Some(Arc::new(store))
        }
        Err(e) => {
            warn!(error = %e, "Failed to initialise file store");
            None
        }
    }
}

fn init_rustfs_admin() -> Option<Arc<RustFsAdminClient>> {
    if std::env::var("S3_ENDPOINT").is_ok() {
        Some(Arc::new(RustFsAdminClient::from_env()))
    } else {
        warn!("S3_ENDPOINT not set — RustFS admin client disabled");
        None
    }
}

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

    let default_binding = LlmBinding {
        provider: "anthropic".into(),
        model: "claude-haiku-4-5".into(),
    };
    let aliases = HashMap::new();
    LlmRegistry::new(providers, aliases, default_binding)
}

fn build_job_registry(
    ctx: Arc<JobContext>,
    billing: Option<StdArc<dyn BillingProvider>>,
) -> Arc<JobRegistry> {
    let ctx = if let Some(b) = billing {
        Arc::new(Arc::unwrap_or_clone(ctx).with_billing(b))
    } else {
        ctx
    };
    let mut registry = JobRegistry::new(ctx);
    registry.register_scheduled(CapabilityHealthCheckJob);
    registry.register_scheduled(AuditLogCleanupJob);
    registry.register_scheduled(LagoReconcileJob);
    registry.register_scheduled(RustFsKeyRotationJob);
    registry.register_scheduled(TenantBucketMigrationJob);
    registry.register_background(VideoTranscriptionJob);
    Arc::new(registry)
}
