use crate::capabilities::job_backed::transcribe_video_provider;
use crate::metrics::{RouterMetrics, RustFsMetrics};
use crate::mw::{RateLimiter, RouterQuotaConfig};
use crate::routes::admin_devices::DeviceToken;
use agent_core::identity::IdentityManager;
use agent_core::llm::providers::anthropic::AnthropicProvider;
use agent_core::realtime::{InvalidationBus, new_invalidation_bus};
use agent_core::{
    ArtifactBridge, BulkCapabilityFactory, CapabilityAdmin, CapabilityDiscovery,
    CapabilityRegistry, CapabilitySpecFactory, ConversationService, CredentialStore,
    DefaultConversationService, EmbeddingService, LlmRegistry, ManifestWatcher, NamespaceFilter,
    NativeStorageFactory, NoopEmbeddingService, QdrantVectorStore, RealtimeService,
    RedbMetadataStore, RustFsContentStore, SemanticCapabilityRouter, SemanticRouterConfig,
    StorageQuotaService, TenantOnboardingService, TenantStorageFactory, build_admin,
};
use agent_core::{LegacyIdentityProvider, ZitadelCacheStats, ZitadelProvider};
use base64::Engine as _;
use billing_core::{BillingProvider, LagoProvider, PlanCatalog, QuotaChecker};
use common::audit::AuditStore;
use common::memory::{
    InMemoryAuditStore, InMemoryThreadStore, InMemoryWorkspaceContent, InMemoryWorkspaceStore,
    ThreadStore, WorkspaceContentStore, WorkspaceStore,
};
use jobs::jobs::{
    AuditLogCleanupJob, CapabilityHealthCheckJob, LagoReconcileJob, RustFsKeyRotationJob,
    TenantBucketMigrationJob, VideoTranscriptionJob,
};
use jobs::{JobAdmin, JobContext, JobExecutor, JobRegistry};
use object_store::ObjectStore;
use object_store::aws::AmazonS3Builder;
use rustfs_admin::RustFsAdminClient;
use std::collections::HashMap;
use std::sync::Arc as StdArc;
use std::sync::{Arc, Mutex};
#[cfg(not(feature = "local-embeddings"))]
use tracing::error;
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
    /// Router-decision metrics (set by `main.rs` at boot; None in test mode).
    pub router_metrics: Option<Arc<RouterMetrics>>,
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
    /// Kept alive so the bulk-loaded capability cards stay registered after boot;
    /// readers should go through `state.registry` rather than this field.
    #[allow(dead_code)]
    pub capability_spec_factory: Option<Arc<CapabilitySpecFactory>>,
    pub artifact_bridge: Option<Arc<ArtifactBridge>>,
    /// Broadcast channel for workspace invalidation events (PR 3.A).
    /// Emit to this when workspace content changes so SSE clients can revalidate.
    pub invalidation_bus: InvalidationBus,
    /// Identity provider (legacy HMAC/JWT or Zitadel OIDC).
    pub identity: StdArc<dyn IdentityManager>,
    /// Hot-reload watcher for capability TOML manifests. Kept alive for the process lifetime —
    /// dropping it stops the notify thread that drives manifest hot-reload.
    #[allow(dead_code)]
    pub manifest_watcher: Option<ManifestWatcher>,
    /// Token cache counters when Zitadel is the active provider; None for legacy.
    pub zitadel_cache_stats: Option<StdArc<ZitadelCacheStats>>,
    /// Billing provider (Lago). None when LAGO_API_KEY not configured.
    pub billing: Option<StdArc<dyn BillingProvider>>,
    /// In-process quota checker.
    pub quota: Option<StdArc<QuotaChecker>>,
    /// Plan catalog loaded at boot.
    pub plan_catalog: StdArc<PlanCatalog>,
}

impl AppState {
    /// Validate startup configuration directly from environment variables.
    ///
    /// This mode intentionally avoids constructing heavy runtime resources
    /// (redb/Qdrant/RustFS) so it can be used as a fast preflight check.
    pub fn validate_env_contracts() -> common::error::Result<()> {
        let mut errors: Vec<String> = Vec::new();
        let is_prod = is_production_env();

        let require = |key: &str, errors: &mut Vec<String>| {
            if std::env::var_os(key).is_none_or(|v| v.is_empty()) {
                errors.push(format!("missing required env var: {key}"));
            }
        };

        // Always validate canonical aliases used by chain manifests.
        let llm = build_llm_registry();
        for alias in ["smart", "opus", "haiku", "fast", "cheap"] {
            if llm.resolve_binding(alias, None).is_err() {
                errors.push(format!("LLM alias binding is unresolved: {alias}"));
            }
        }

        match std::env::var("EMBEDDING_BACKEND").as_deref() {
            Ok("local") | Err(_) => {}
            Ok(other) => errors.push(format!(
                "unknown EMBEDDING_BACKEND={other} (supported: local)"
            )),
        }

        if let Ok(model) = std::env::var("EMBEDDING_LOCAL_MODEL")
            && agent_core::indexing::embedding_service::EmbeddingModel::from_name(&model).is_none()
        {
            errors.push(format!(
                "unknown EMBEDDING_LOCAL_MODEL={model} (supported: multilingual-e5-large, bge-small-en-v1.5, bge-m3, nomic-embed-text-v1.5, all-minilm-l6-v2)"
            ));
        }

        if is_prod {
            match std::env::var("WEB_ORIGIN") {
                Ok(raw) if raw.split(',').any(|origin| !origin.trim().is_empty()) => {
                    let invalid_origin = raw.split(',').map(str::trim).find(|origin| {
                        origin == &"*"
                            || origin.starts_with("http://localhost")
                            || origin.starts_with("https://localhost")
                            || origin.starts_with("http://127.0.0.1")
                            || origin.starts_with("https://127.0.0.1")
                    });
                    if let Some(origin) = invalid_origin {
                        errors.push(format!(
                            "WEB_ORIGIN contains development or wildcard origin in production: {origin}"
                        ));
                    }
                }
                _ => errors.push(
                    "WEB_ORIGIN must be explicitly set in production; refusing localhost CORS fallback"
                        .to_string(),
                ),
            }
        }

        if is_prod {
            require("JWT_SECRET", &mut errors);
            require("UI_SESSION_KEY", &mut errors);
            require("SUPER_ADMIN_EMAILS", &mut errors);
            require("PLATFORM_ADMIN_TOKEN", &mut errors);
            require("QDRANT_URL", &mut errors);
            require("RUSTFS_WEBHOOK_SECRET", &mut errors);

            if std::env::var_os("LAGO_API_KEY").is_none_or(|v| v.is_empty()) {
                errors.push(
                    "LAGO_API_KEY missing: billing provider is disabled in production".to_string(),
                );
            }

            let auth_provider = std::env::var("CONUSAI_AUTH_PROVIDER")
                .unwrap_or_else(|_| "legacy".to_string())
                .to_lowercase();
            let allow_legacy_in_prod = std::env::var("CONUSAI_ALLOW_LEGACY_AUTH_IN_PROD")
                .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
                .unwrap_or(false);
            if auth_provider != "zitadel" && !allow_legacy_in_prod {
                errors.push(
                    "legacy auth is disabled in production; set CONUSAI_AUTH_PROVIDER=zitadel or explicitly override with CONUSAI_ALLOW_LEGACY_AUTH_IN_PROD=1"
                        .to_string(),
                );
            }

            // Per-tenant IAM credential encryption must be explicit in production
            // when RustFS integration is configured.
            if std::env::var_os("RUSTFS_ENDPOINT").is_some() {
                match std::env::var("RUSTFS_IAM_ENC_KEY") {
                    Ok(v) => {
                        match base64::engine::general_purpose::STANDARD.decode(v.as_bytes()) {
                            Ok(raw) if raw.len() == 32 => {}
                            Ok(raw) => errors.push(format!(
                                "RUSTFS_IAM_ENC_KEY must decode to 32 bytes, got {}",
                                raw.len()
                            )),
                            Err(e) => errors.push(format!(
                                "RUSTFS_IAM_ENC_KEY must be valid base64: {e}"
                            )),
                        }
                    }
                    Err(_) => errors.push(
                        "missing required env var: RUSTFS_IAM_ENC_KEY (per-tenant IAM encryption key)"
                            .to_string(),
                    ),
                }
            }

            if std::env::var("CONUSAI_AUTH_PROVIDER")
                .map(|v| v == "zitadel")
                .unwrap_or(false)
            {
                for key in [
                    "ZITADEL_DOMAIN",
                    "ZITADEL_AUDIENCE",
                    "ZITADEL_INTROSPECTION_CLIENT_ID",
                    "ZITADEL_INTROSPECTION_CLIENT_SECRET",
                    "ZITADEL_MGMT_PAT",
                ] {
                    require(key, &mut errors);
                }
            }
        }

        if errors.is_empty() {
            return Ok(());
        }

        let mut msg = String::from("startup config validation failed:\n");
        for e in errors {
            msg.push_str(" - ");
            msg.push_str(&e);
            msg.push('\n');
        }
        Err(common::error::ConusAiError::Config(msg))
    }

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

        let redb_path = std::env::var("REDB_PATH").unwrap_or_else(|_| "/data/conusai.redb".into());
        let metadata_store: Arc<RedbMetadataStore> = RedbMetadataStore::open(&redb_path)
            .map_err(|e| common::error::ConusAiError::Storage(e.to_string()))?;

        // Share the same Database handle — redb v2 forbids two instances on one file.
        let cred_store: Arc<CredentialStore> = Arc::new(
            CredentialStore::new(metadata_store.db())
                .map_err(|e| common::error::ConusAiError::Storage(e.to_string()))?,
        );

        // ── Embedding service (must be before Qdrant so dims are known) ─────
        let embedding_service: Arc<dyn EmbeddingService> = match std::env::var("EMBEDDING_BACKEND")
            .as_deref()
        {
            Ok("local") | Err(_) => {
                #[cfg(feature = "local-embeddings")]
                {
                    let svc = agent_core::LocalEmbeddingService::from_env()?;
                    info!(
                        model = svc.model().name(),
                        dims = svc.dims(),
                        "embedding service: local fastembed"
                    );
                    Arc::new(svc)
                }
                #[cfg(not(feature = "local-embeddings"))]
                {
                    // Visible boot banner on stderr — survives log-level filtering.
                    eprintln!(
                        "\n\
                         ╔════════════════════════════════════════════════════════════════════╗\n\
                         ║  ⚠️  GATEWAY HAS NO EMBEDDINGS                                     ║\n\
                         ║  Semantic router will return ZERO tools every turn.                ║\n\
                         ║  Rebuild: cargo build --features agent-gateway/local-embeddings    ║\n\
                         ╚════════════════════════════════════════════════════════════════════╝\n"
                    );
                    error!(
                        "local-embeddings feature not compiled — semantic router will serve zero \
                         tools. Rebuild with: cargo build --features agent-gateway/local-embeddings"
                    );
                    Arc::new(NoopEmbeddingService)
                }
            }
            Ok(other) => {
                return Err(common::error::ConusAiError::Config(format!(
                    "unknown EMBEDDING_BACKEND={other} (supported: local)"
                )));
            }
        };

        let qdrant_url =
            std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://qdrant:6334".into());
        let vector_store = Arc::new(
            QdrantVectorStore::connect_from_service(&qdrant_url, embedding_service.as_ref())
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

        info!(
            "workspace content: RustFS/S3 object store (per-tenant IAM via TenantStorageFactory)"
        );

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

        let mut registry_raw = CapabilityRegistry::with_all_factories(Arc::clone(&llm));
        // NativeStorageFactory handles ToolKind::Native manifests (storage-workspace,
        // storage-read-text, storage-write-text) loaded from the capabilities directory.
        registry_raw.register_factory(NativeStorageFactory::new(
            Arc::clone(&workspace_store),
            Arc::clone(&workspace_content),
        ));
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

        let registry = Arc::new(Mutex::new(registry_raw));

        let router_cfg = SemanticRouterConfig {
            top_k: std::env::var("SEMANTIC_ROUTER_TOP_K")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(20),
            namespace: NamespaceFilter::Any,
            include_always: vec!["storage-workspace".into()],
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
        let auth_provider =
            std::env::var("CONUSAI_AUTH_PROVIDER").unwrap_or_else(|_| "legacy".into());
        let (identity, zitadel_cache_stats): (
            StdArc<dyn IdentityManager>,
            Option<StdArc<ZitadelCacheStats>>,
        ) = if auth_provider == "zitadel" {
            match ZitadelProvider::from_env() {
                Ok(p) => {
                    info!("identity provider: Zitadel OIDC");
                    let stats = StdArc::clone(&p.stats);
                    (StdArc::new(p) as StdArc<dyn IdentityManager>, Some(stats))
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Zitadel provider init failed — falling back to legacy");
                    (
                        StdArc::new(LegacyIdentityProvider::from_env())
                            as StdArc<dyn IdentityManager>,
                        None,
                    )
                }
            }
        } else {
            info!("identity provider: legacy HMAC/JWT");
            (
                StdArc::new(LegacyIdentityProvider::from_env()) as StdArc<dyn IdentityManager>,
                None,
            )
        };

        // ── Plan catalog ──────────────────────────────────────────────────
        let plan_catalog = StdArc::new(PlanCatalog::load());
        info!(plans = plan_catalog.list().len(), "plan catalog loaded");

        // ── Billing (Lago) — initialized before job registry so reconcile job has it ──
        let (billing, quota): (
            Option<StdArc<dyn BillingProvider>>,
            Option<StdArc<QuotaChecker>>,
        ) = match LagoProvider::from_env() {
            Ok(provider) => {
                info!("billing provider: Lago");
                let quota_checker = StdArc::new(QuotaChecker::new(StdArc::clone(&plan_catalog)));
                (
                    Some(StdArc::new(provider) as StdArc<dyn BillingProvider>),
                    Some(quota_checker),
                )
            }
            Err(e) => {
                warn!(error = %e, "Lago not configured — billing/metering disabled");
                (None, None)
            }
        };

        let s3_endpoint = std::env::var("S3_ENDPOINT").ok();
        let bucket = std::env::var("S3_BUCKET").unwrap_or_else(|_| "workspace".into());
        let mut job_ctx = JobContext::new(Arc::clone(&audit_store), s3_endpoint, Some(bucket));
        if let (Some(ra), Some(cs)) = (rustfs_admin.as_ref(), Some(&cred_store)) {
            job_ctx = job_ctx.with_rustfs(Arc::clone(ra), Arc::clone(cs));
        }
        job_ctx = job_ctx.with_storage(Arc::clone(&tenant_storage), Arc::clone(&workspace_store));
        let job_ctx = Arc::new(job_ctx);
        let job_registry = build_job_registry(job_ctx, billing.clone());
        let job_executor = JobExecutor::new(Arc::clone(&job_registry));
        let job_admin = Arc::new(JobAdmin::new(
            Arc::clone(&job_registry),
            Arc::clone(&job_executor),
        ));

        let artifact_bridge = file_store
            .as_ref()
            .map(|fs| ArtifactBridge::new(Arc::clone(fs), Arc::clone(&workspace_content)));

        let realtime_service = RealtimeService::new();

        // Register job-backed capabilities that need Arc<JobExecutor>.
        {
            use agent_core::capabilities::provider::CapabilityProvider;
            let provider: Arc<dyn CapabilityProvider> =
                Arc::new(transcribe_video_provider(&job_executor));
            registry.lock().unwrap().register_provider(provider);
            info!("transcribe-video capability registered");
        }

        // Start hot-reload watcher (250 ms debounce, non-fatal if notify unavailable).
        let manifest_watcher = match ManifestWatcher::start(
            Arc::clone(&registry),
            Some(Arc::clone(&realtime_service)),
        ) {
            Ok(w) => {
                info!("manifest hot-reload watcher started");
                Some(w)
            }
            Err(e) => {
                warn!(error = %e, "manifest watcher unavailable — hot-reload disabled");
                None
            }
        };

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
            router_metrics: None,
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
            invalidation_bus: new_invalidation_bus(),
            identity,
            manifest_watcher,
            zitadel_cache_stats,
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
        let thread_store: Arc<dyn ThreadStore> = Arc::new(InMemoryThreadStore::new());
        let workspace_store: Arc<dyn WorkspaceStore> = Arc::new(InMemoryWorkspaceStore::new());
        let workspace_content: Arc<dyn WorkspaceContentStore> =
            Arc::new(InMemoryWorkspaceContent::new());

        let mut registry = CapabilityRegistry::with_default_factories(Arc::clone(&llm));
        registry.register_factory(NativeStorageFactory::new(
            Arc::clone(&workspace_store),
            Arc::clone(&workspace_content),
        ));
        CapabilityDiscovery::from_env().discover_into(&mut registry)?;

        let conversation_service: Arc<dyn ConversationService> =
            Arc::new(DefaultConversationService {
                thread_store: Arc::clone(&thread_store),
                workspace_store: Arc::clone(&workspace_store),
            });

        let audit_store: Arc<dyn AuditStore> = Arc::new(InMemoryAuditStore::new());

        let registry = Arc::new(Mutex::new(registry));
        let tool_admin = Arc::new(build_admin(Arc::clone(&registry), Arc::clone(&audit_store)));

        let embedding_service: Arc<dyn EmbeddingService> = Arc::new(NoopEmbeddingService);
        let vector_store = Arc::new(QdrantVectorStore::noop());
        let semantic_router = SemanticCapabilityRouter::new(
            Arc::clone(&registry),
            Arc::clone(&vector_store),
            Arc::clone(&embedding_service),
            SemanticRouterConfig {
                include_always: vec!["storage-workspace".into()],
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
            router_metrics: None,
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
            invalidation_bus: new_invalidation_bus(),
            identity,
            manifest_watcher: None,
            zitadel_cache_stats: None,
            billing: None,
            quota: None,
            plan_catalog,
        })
    }
}

fn is_production_env() -> bool {
    std::env::var("CONUSAI_ENV")
        .map(|v| matches!(v.as_str(), "production" | "prod"))
        .unwrap_or(false)
        || std::env::var("RUST_ENV")
            .map(|v| matches!(v.as_str(), "production" | "prod"))
            .unwrap_or(false)
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
    let endpoint = std::env::var("S3_ENDPOINT").unwrap_or_else(|_| "http://rustfs:9000".into());
    let bucket = std::env::var("S3_BUCKET").unwrap_or_else(|_| "workspace".into());
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

    // Canonical aliases consumed by chain TOML manifests + plan.orchestrate.
    // Override via env (e.g. `LLM_ALIAS_SMART=claude-opus-4-7`) for cost tuning.
    let alias_specs: &[(&str, &str, &str)] = &[
        ("smart", "anthropic", "claude-sonnet-4-6"),
        ("opus", "anthropic", "claude-opus-4-7"),
        ("haiku", "anthropic", "claude-haiku-4-5"),
        ("fast", "anthropic", "claude-haiku-4-5"),
        ("cheap", "anthropic", "claude-haiku-4-5"),
    ];
    let mut aliases: HashMap<String, LlmBinding> = HashMap::new();
    for (alias, provider, default_model) in alias_specs {
        let env_key = format!("LLM_ALIAS_{}", alias.to_uppercase());
        let model = std::env::var(&env_key).unwrap_or_else(|_| (*default_model).to_string());
        aliases.insert(
            (*alias).to_string(),
            LlmBinding {
                provider: (*provider).to_string(),
                model,
            },
        );
    }

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

#[cfg(test)]
mod tests {
    use super::AppState;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn set_min_prod_env() {
        // Safety: tests mutate process env under env_lock to avoid races.
        unsafe {
            std::env::set_var("CONUSAI_ENV", "production");
            std::env::set_var("JWT_SECRET", "test-jwt-secret");
            std::env::set_var("UI_SESSION_KEY", "test-ui-session-key");
            std::env::set_var("SUPER_ADMIN_EMAILS", "admin@example.com");
            std::env::set_var("PLATFORM_ADMIN_TOKEN", "test-admin-token");
            std::env::set_var("QDRANT_URL", "http://localhost:6334");
            std::env::set_var("RUSTFS_WEBHOOK_SECRET", "test-webhook-secret");
            std::env::set_var("LAGO_API_KEY", "test-lago-key");
            std::env::set_var(
                "WEB_ORIGIN",
                "https://app.example.com,https://tauri.localhost",
            );
            std::env::remove_var("RUSTFS_ENDPOINT");
        }
    }

    #[test]
    fn validate_env_contracts_rejects_legacy_auth_in_production() {
        let _guard = env_lock().lock().expect("env lock");
        set_min_prod_env();
        // Safety: tests mutate process env under env_lock to avoid races.
        unsafe {
            std::env::set_var("CONUSAI_AUTH_PROVIDER", "legacy");
            std::env::remove_var("CONUSAI_ALLOW_LEGACY_AUTH_IN_PROD");
        }

        let err = AppState::validate_env_contracts().expect_err("should reject legacy in prod");
        let msg = err.to_string();
        assert!(
            msg.contains("legacy auth is disabled in production"),
            "{msg}"
        );
    }

    #[test]
    fn validate_env_contracts_allows_legacy_auth_with_explicit_override() {
        let _guard = env_lock().lock().expect("env lock");
        set_min_prod_env();
        // Safety: tests mutate process env under env_lock to avoid races.
        unsafe {
            std::env::set_var("CONUSAI_AUTH_PROVIDER", "legacy");
            std::env::set_var("CONUSAI_ALLOW_LEGACY_AUTH_IN_PROD", "1");
        }

        let result = AppState::validate_env_contracts();
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn validate_env_contracts_rejects_unknown_embedding_backend_and_model() {
        let _guard = env_lock().lock().expect("env lock");
        set_min_prod_env();
        // Safety: tests mutate process env under env_lock to avoid races.
        unsafe {
            std::env::set_var("EMBEDDING_BACKEND", "openai");
            std::env::set_var("EMBEDDING_LOCAL_MODEL", "not-a-real-model");
        }

        let err =
            AppState::validate_env_contracts().expect_err("should reject bad embedding config");
        let msg = err.to_string();
        assert!(msg.contains("unknown EMBEDDING_BACKEND=openai"), "{msg}");
        assert!(
            msg.contains("unknown EMBEDDING_LOCAL_MODEL=not-a-real-model"),
            "{msg}"
        );
    }

    #[test]
    fn validate_env_contracts_rejects_missing_web_origin_in_production() {
        let _guard = env_lock().lock().expect("env lock");
        set_min_prod_env();
        unsafe {
            std::env::remove_var("WEB_ORIGIN");
        }

        let err = AppState::validate_env_contracts().expect_err("should reject missing WEB_ORIGIN");
        let msg = err.to_string();
        assert!(
            msg.contains("WEB_ORIGIN must be explicitly set in production"),
            "{msg}"
        );
    }

    #[test]
    fn validate_env_contracts_rejects_localhost_web_origin_in_production() {
        let _guard = env_lock().lock().expect("env lock");
        set_min_prod_env();
        unsafe {
            std::env::set_var(
                "WEB_ORIGIN",
                "https://app.example.com,http://localhost:3000",
            );
        }

        let err =
            AppState::validate_env_contracts().expect_err("should reject localhost WEB_ORIGIN");
        let msg = err.to_string();
        assert!(
            msg.contains("WEB_ORIGIN contains development or wildcard origin in production"),
            "{msg}"
        );
    }
}
