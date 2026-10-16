use crate::auth::ApiKeyEntry;
use crate::capabilities::job_backed::transcribe_video_provider;
use crate::config::GatewayConfig;
use crate::metrics::{RouterMetrics, RustFsMetrics};
use crate::mw::{RateLimiter, RouterQuotaConfig};
use crate::plans::PlanResolver;
use agent_core::capabilities::object_source::TenantObjectSource;
use agent_core::llm::providers::anthropic::AnthropicProvider;
use agent_core::realtime::{InvalidationBus, new_invalidation_bus};
use agent_core::{
    ArtifactBridge, CapabilityAdmin, CapabilityDiscovery, CapabilityRegistry, ConversationService,
    CredentialStore, DefaultConversationService, EmbeddingService, LlmRegistry, ManifestWatcher,
    NamespaceFilter, NativeStorageFactory, QdrantVectorStore, RealtimeService, RedbMetadataStore,
    RustFsContentStore, SemanticCapabilityRouter, SemanticRouterConfig, StorageQuotaService,
    TenantOnboardingService, TenantStorageFactory, TokenVerifier, ZitadelVerifier, build_admin,
};
use billing_core::lago::LagoProvider;
use billing_core::{BillingProvider, PlanCatalog, QuotaChecker};
use common::audit::AuditStore;
use common::memory::{ThreadStore, WorkspaceContentStore, WorkspaceStore};
use jobs::jobs::{
    AuditLogCleanupJob, CapabilityHealthCheckJob, LagoReconcileJob, RustFsKeyRotationJob,
    TenantBucketMigrationJob, VideoTranscriptionJob,
};
use jobs::{JobAdmin, JobContext, JobExecutor, JobRegistry};
use object_store::ObjectStore;
use object_store::aws::AmazonS3Builder;
use rustfs_admin::{RustFsAdminClient, S3Config};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::info;

pub struct AppState {
    pub registry: Arc<Mutex<CapabilityRegistry>>,
    pub rate_limiter: RateLimiter,
    pub llm: Arc<LlmRegistry>,
    pub anthropic_api_key: String,
    /// Bearer-token verifier (Zitadel).
    pub verifier: Arc<dyn TokenVerifier>,
    /// Machine-client keys (`API_KEYS`).
    pub api_keys: Vec<ApiKeyEntry>,
    /// Server-side plan tier per tenant (Lago, cached).
    pub plans: PlanResolver,
    /// `PLATFORM_ADMIN_TOKEN` — capability self-registration and `/metrics`.
    pub platform_admin_token: String,
    /// RustFS admin client (root credentials — bootstrap and IAM provisioning).
    pub rustfs_admin: Option<Arc<RustFsAdminClient>>,
    /// Per-tenant credential store (encrypted in redb).
    pub cred_store: Option<Arc<CredentialStore>>,
    /// Per-tenant storage factory — single source of truth for tenant S3 operations.
    pub tenant_storage: Option<Arc<TenantStorageFactory>>,
    /// Tenant onboarding service — provisions IAM + default workspace root.
    pub onboarding: Option<Arc<TenantOnboardingService>>,
    pub storage_quota: Arc<StorageQuotaService>,
    pub presign_ttl: Duration,
    /// HMAC secret of the RustFS bucket-notification webhook, when configured.
    pub rustfs_webhook_secret: Option<String>,
    pub rustfs_metrics: Option<Arc<RustFsMetrics>>,
    pub router_metrics: Option<Arc<RouterMetrics>>,
    /// Per-tenant single-flight mutex for the onboarding safety net in the `tree` route.
    pub onboarding_guards: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>>,
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
    pub artifact_bridge: Option<Arc<ArtifactBridge>>,
    /// Broadcast channel for workspace invalidation events.
    pub invalidation_bus: InvalidationBus,
    /// Hot-reload watcher for capability manifests; dropping it stops hot reload.
    #[allow(dead_code)]
    pub manifest_watcher: Option<ManifestWatcher>,
    /// Billing provider (Lago). None when `LAGO_API_KEY` is not configured.
    pub billing: Option<Arc<dyn BillingProvider>>,
    /// In-process quota checker (present with billing).
    pub quota: Option<Arc<QuotaChecker>>,
    pub plan_catalog: Arc<PlanCatalog>,
}

fn storage_err(e: impl std::fmt::Display) -> common::error::ConusAiError {
    common::error::ConusAiError::Storage(e.to_string())
}

impl AppState {
    pub async fn from_config(cfg: &GatewayConfig) -> anyhow::Result<Self> {
        let llm = Arc::new(build_llm_registry()?);

        // ── Persistent stores (redb + Qdrant + RustFS) ───────────────────────
        let metadata_store = RedbMetadataStore::open(&cfg.redb_path).map_err(storage_err)?;
        // Share the same Database handle — redb forbids two instances on one file.
        let cred_store = Arc::new(
            CredentialStore::new(metadata_store.db(), &cfg.iam_enc_key).map_err(storage_err)?,
        );

        let embedding_service = build_embedding_service(cfg)?;
        let vector_store = Arc::new(
            QdrantVectorStore::connect_from_service(&cfg.qdrant, embedding_service.as_ref())
                .await
                .map_err(storage_err)?,
        );

        let thread_store: Arc<dyn ThreadStore> = metadata_store.clone();
        let audit_store: Arc<dyn AuditStore> = metadata_store.clone();
        let workspace_store: Arc<dyn WorkspaceStore> = metadata_store.clone();

        let tenant_storage = Arc::new(TenantStorageFactory::new(
            Arc::clone(&cred_store),
            Arc::clone(&audit_store),
            &cfg.s3,
        ));
        let workspace_content: Arc<dyn WorkspaceContentStore> =
            RustFsContentStore::new(Arc::clone(&tenant_storage));
        let file_store = build_root_store(&cfg.s3)?;
        let rustfs_admin = Arc::new(RustFsAdminClient::from_config(&cfg.s3));
        let onboarding = TenantOnboardingService::new(
            Arc::clone(&workspace_store),
            Arc::clone(&tenant_storage),
            Arc::clone(&cred_store),
            Arc::clone(&rustfs_admin),
        );
        let storage_quota =
            StorageQuotaService::new(Arc::clone(&tenant_storage), cfg.storage_quotas);

        // ── Capabilities ──────────────────────────────────────────────────────
        let objects: Arc<dyn TenantObjectSource> = tenant_storage.clone();
        let mut registry_raw = CapabilityRegistry::with_all_factories(Arc::clone(&llm), objects)?;
        registry_raw.register_factory(NativeStorageFactory::new(
            Arc::clone(&workspace_store),
            Arc::clone(&workspace_content),
        ));
        CapabilityDiscovery::new(vec![cfg.capabilities_dir.clone()])
            .discover_into(&mut registry_raw)?;
        let registry = Arc::new(Mutex::new(registry_raw));

        let semantic_router = SemanticCapabilityRouter::new(
            Arc::clone(&registry),
            Arc::clone(&vector_store),
            Arc::clone(&embedding_service),
            SemanticRouterConfig {
                top_k: cfg.semantic_top_k,
                namespace: NamespaceFilter::Any,
                include_always: vec!["storage-workspace".into()],
                ..Default::default()
            },
        );

        let conversation_service: Arc<dyn ConversationService> =
            Arc::new(DefaultConversationService {
                thread_store: Arc::clone(&thread_store),
                workspace_store: Arc::clone(&workspace_store),
            });
        let tool_admin = Arc::new(build_admin(
            cfg.capabilities_dir.clone(),
            Arc::clone(&registry),
            Arc::clone(&audit_store),
        ));

        // ── Identity ──────────────────────────────────────────────────────────
        let verifier: Arc<dyn TokenVerifier> = Arc::new(
            ZitadelVerifier::new(cfg.zitadel.clone())
                .map_err(|e| anyhow::anyhow!("Zitadel: {e}"))?,
        );
        info!(issuer = %cfg.zitadel.issuer, api_keys = cfg.api_keys.len(), "identity: Zitadel + API keys");

        // ── Billing ───────────────────────────────────────────────────────────
        let plan_catalog = Arc::new(PlanCatalog::load());
        let (billing, quota): (Option<Arc<dyn BillingProvider>>, Option<Arc<QuotaChecker>>) =
            match &cfg.lago {
                Some(lago) => {
                    info!("billing provider: Lago");
                    (
                        Some(Arc::new(LagoProvider::new(lago.clone()))),
                        Some(Arc::new(QuotaChecker::new(Arc::clone(&plan_catalog)))),
                    )
                }
                None => {
                    info!(
                        "billing not configured (LAGO_API_KEY unset) — every tenant is on the Free plan"
                    );
                    (None, None)
                }
            };

        // ── Jobs ──────────────────────────────────────────────────────────────
        let job_ctx = JobContext::new(Arc::clone(&audit_store))
            .with_rustfs(Arc::clone(&rustfs_admin), Arc::clone(&cred_store))
            .with_storage(Arc::clone(&tenant_storage), Arc::clone(&workspace_store));
        let job_registry = build_job_registry(Arc::new(job_ctx), billing.clone());
        let job_executor = JobExecutor::new(Arc::clone(&job_registry));
        let job_admin = Arc::new(JobAdmin::new(
            Arc::clone(&job_registry),
            Arc::clone(&job_executor),
        ));
        registry
            .lock()
            .unwrap()
            .register_provider(Arc::new(transcribe_video_provider(&job_executor)));

        let artifact_bridge = ArtifactBridge::new(
            Arc::clone(&file_store),
            Arc::clone(&workspace_content),
            cfg.public_base_url.clone(),
        );

        let realtime_service = RealtimeService::new();
        let manifest_watcher = match ManifestWatcher::start(
            Arc::clone(&registry),
            vec![cfg.capabilities_dir.clone()],
            Some(Arc::clone(&realtime_service)),
        ) {
            Ok(w) => Some(w),
            Err(e) => {
                tracing::warn!(error = %e, "manifest watcher unavailable — hot-reload disabled");
                None
            }
        };

        Ok(Self {
            registry,
            rate_limiter: RateLimiter::new(),
            llm,
            anthropic_api_key: cfg.anthropic_api_key.clone(),
            verifier,
            api_keys: cfg.api_keys.clone(),
            plans: PlanResolver::new(billing.clone()),
            platform_admin_token: cfg.platform_admin_token.clone(),
            rustfs_admin: Some(rustfs_admin),
            cred_store: Some(cred_store),
            tenant_storage: Some(tenant_storage),
            onboarding: Some(onboarding),
            storage_quota,
            presign_ttl: cfg.presign_ttl,
            rustfs_webhook_secret: cfg.rustfs_webhook_secret.clone(),
            rustfs_metrics: None,
            router_metrics: None,
            onboarding_guards: Arc::new(Mutex::new(HashMap::new())),
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
            artifact_bridge: Some(artifact_bridge),
            invalidation_bus: new_invalidation_bus(),
            manifest_watcher,
            billing,
            quota,
            plan_catalog,
        })
    }
}

#[cfg(feature = "local-embeddings")]
fn build_embedding_service(cfg: &GatewayConfig) -> anyhow::Result<Arc<dyn EmbeddingService>> {
    let svc = agent_core::LocalEmbeddingService::new(&agent_core::LocalEmbeddingConfig {
        model: cfg.embedding.model,
        cache_dir: cfg.embedding.cache_dir.clone(),
        max_batch: cfg.embedding.max_batch,
    })?;
    info!(
        model = svc.model().name(),
        dims = svc.dims(),
        "embedding service: local fastembed"
    );
    Ok(Arc::new(svc))
}

#[cfg(not(feature = "local-embeddings"))]
fn build_embedding_service(_cfg: &GatewayConfig) -> anyhow::Result<Arc<dyn EmbeddingService>> {
    tracing::error!(
        "built without the `local-embeddings` feature — the semantic router serves zero tools"
    );
    Ok(Arc::new(agent_core::NoopEmbeddingService))
}

/// Root-credential store for the shared bucket — artifact materialisation only.
/// Tenant reads and writes always go through `TenantStorageFactory`.
fn build_root_store(s3: &S3Config) -> anyhow::Result<Arc<dyn ObjectStore>> {
    let store = AmazonS3Builder::new()
        .with_endpoint(s3.endpoint.trim_end_matches('/'))
        .with_bucket_name(&s3.bucket)
        .with_access_key_id(&s3.access_key_id)
        .with_secret_access_key(&s3.secret_access_key)
        .with_allow_http(true)
        .with_region("us-east-1")
        .build()?;
    Ok(Arc::new(store))
}

fn build_llm_registry() -> anyhow::Result<LlmRegistry> {
    use agent_core::llm::types::LlmBinding;

    let mut providers: HashMap<String, Arc<dyn agent_core::CompletionProvider>> = HashMap::new();
    providers.insert("anthropic".into(), Arc::new(AnthropicProvider::from_env()?));

    let default_binding = LlmBinding {
        provider: "anthropic".into(),
        model: "claude-haiku-4-5".into(),
    };

    // Canonical aliases consumed by chain manifests and the plan allowlists.
    // Override the model per alias with `LLM_ALIAS_<ALIAS>` (e.g. `LLM_ALIAS_SMART`).
    let alias_specs: &[(&str, &str)] = &[
        ("smart", "claude-sonnet-4-6"),
        ("opus", "claude-opus-4-7"),
        ("haiku", "claude-haiku-4-5"),
        ("fast", "claude-haiku-4-5"),
        ("cheap", "claude-haiku-4-5"),
    ];
    let mut aliases: HashMap<String, LlmBinding> = HashMap::new();
    for (alias, default_model) in alias_specs {
        let env_key = format!("LLM_ALIAS_{}", alias.to_uppercase());
        let model = common::env::var(&env_key).unwrap_or_else(|| (*default_model).to_string());
        aliases.insert(
            (*alias).to_string(),
            LlmBinding {
                provider: "anthropic".into(),
                model,
            },
        );
    }

    Ok(LlmRegistry::new(providers, aliases, default_binding))
}

fn build_job_registry(
    ctx: Arc<JobContext>,
    billing: Option<Arc<dyn BillingProvider>>,
) -> Arc<JobRegistry> {
    let ctx = match billing {
        Some(b) => Arc::new(Arc::unwrap_or_clone(ctx).with_billing(b)),
        None => ctx,
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

/// In-memory state for router-level tests: no Qdrant, RustFS, Lago or Zitadel.
#[cfg(test)]
pub mod test_support {
    use super::*;
    use agent_core::NoopEmbeddingService;
    use agent_core::capabilities::object_source::NoObjectSource;
    use common::memory::{
        InMemoryAuditStore, InMemoryThreadStore, InMemoryWorkspaceContent, InMemoryWorkspaceStore,
    };

    pub const TEST_PLATFORM_TOKEN: &str = "test-platform-admin-token-0123456789abcdef";

    pub fn state(verifier: Arc<dyn TokenVerifier>, api_keys: Vec<ApiKeyEntry>) -> AppState {
        let llm = Arc::new(LlmRegistry::new(
            HashMap::new(),
            HashMap::new(),
            agent_core::LlmBinding {
                provider: "anthropic".into(),
                model: "claude-haiku-4-5".into(),
            },
        ));
        let thread_store: Arc<dyn ThreadStore> = Arc::new(InMemoryThreadStore::new());
        let workspace_store: Arc<dyn WorkspaceStore> = Arc::new(InMemoryWorkspaceStore::new());
        let workspace_content: Arc<dyn WorkspaceContentStore> =
            Arc::new(InMemoryWorkspaceContent::new());
        let audit_store: Arc<dyn AuditStore> = Arc::new(InMemoryAuditStore::new());

        let mut registry =
            CapabilityRegistry::with_default_factories(Arc::clone(&llm), Arc::new(NoObjectSource))
                .unwrap();
        registry.register_factory(NativeStorageFactory::new(
            Arc::clone(&workspace_store),
            Arc::clone(&workspace_content),
        ));
        let registry = Arc::new(Mutex::new(registry));
        let tool_admin = Arc::new(build_admin(
            std::env::temp_dir().join("gateway-test-capabilities"),
            Arc::clone(&registry),
            Arc::clone(&audit_store),
        ));
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
        let job_registry =
            build_job_registry(Arc::new(JobContext::new(Arc::clone(&audit_store))), None);
        let job_executor = JobExecutor::new(Arc::clone(&job_registry));
        let job_admin = Arc::new(JobAdmin::new(
            Arc::clone(&job_registry),
            Arc::clone(&job_executor),
        ));

        // Quotas disabled; the factory is never asked for credentials.
        let db = Arc::new(
            redb::Database::builder()
                .create_with_backend(redb::backends::InMemoryBackend::new())
                .unwrap(),
        );
        let cs = Arc::new(CredentialStore::new(db, &[7u8; 32]).unwrap());
        let s3 = S3Config {
            endpoint: "http://127.0.0.1:9".into(),
            access_key_id: "unused".into(),
            secret_access_key: "unused".into(),
            bucket: "unused".into(),
        };
        let storage_quota = StorageQuotaService::new(
            Arc::new(TenantStorageFactory::new(cs, Arc::clone(&audit_store), &s3)),
            false,
        );

        AppState {
            registry,
            rate_limiter: RateLimiter::new(),
            llm,
            anthropic_api_key: String::new(),
            verifier,
            api_keys,
            plans: PlanResolver::new(None),
            platform_admin_token: TEST_PLATFORM_TOKEN.into(),
            rustfs_admin: None,
            cred_store: None,
            tenant_storage: None,
            onboarding: None,
            storage_quota,
            presign_ttl: Duration::from_secs(900),
            rustfs_webhook_secret: None,
            rustfs_metrics: None,
            router_metrics: None,
            onboarding_guards: Arc::new(Mutex::new(HashMap::new())),
            thread_store: Arc::clone(&thread_store),
            audit_store,
            workspace_store: Arc::clone(&workspace_store),
            workspace_content,
            conversation_service: Arc::new(DefaultConversationService {
                thread_store,
                workspace_store,
            }),
            tool_admin,
            job_registry,
            job_executor,
            job_admin,
            embedding_service,
            vector_store,
            realtime_service: RealtimeService::new(),
            semantic_router,
            router_quota: RouterQuotaConfig::default(),
            artifact_bridge: None,
            invalidation_bus: new_invalidation_bus(),
            manifest_watcher: None,
            billing: None,
            quota: None,
            plan_catalog: Arc::new(PlanCatalog::default()),
        }
    }
}
