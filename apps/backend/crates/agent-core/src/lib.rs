pub mod agent;
pub mod bridge;
pub mod capabilities;
pub mod chains;
pub mod context;
pub mod identity;
pub mod indexing;
pub mod llm;
pub mod memory;
pub mod model_catalog;
pub mod prompt;
pub mod realtime;
pub mod store;
pub mod vector_store;
pub mod workspace_ops;

pub use agent::builder::{Agent, AgentBuilder};
pub use agent::hooks::{OrchestrationHook, PermissionHook, TracingHook};
pub use agent::runtime::map_rig_error;
pub use bridge::ArtifactBridge;
pub use capabilities::admin::{
    AdminLimits, CapabilityAdmin, CapabilitySummary, CreateCapabilityRequest, TestInvokeRequest,
    TestInvokeResponse, UpdateCapabilityRequest, build_admin,
};
pub use capabilities::card::CapabilityCard;
pub use capabilities::discovery::{CapabilityDiscovery, ManifestWatcher};
pub use capabilities::executor::{PlanStep, StepResult, run_plan};
pub use capabilities::namespace::NamespaceFilter;
pub use capabilities::provider::{BulkCapabilityFactory, CapabilityFactory};
pub use capabilities::providers::capability_spec::CapabilitySpecFactory;
pub use capabilities::providers::job_backed::{JobBackedProvider, JobDispatch};
pub use capabilities::providers::native_storage::NativeStorageFactory;
pub use capabilities::registry::CapabilityRegistry;
pub use capabilities::semantic_router::{
    AttachmentHint, RouterMetrics, SemanticCapabilityRouter, SemanticRouterConfig,
};
pub use capabilities::store::{FilesystemStore, RegisteredToolState, RegisteredToolStore};
pub use capabilities::validator::{
    RegisteredToolValidationError, RegisteredToolValidator, ValidationReport,
};
pub use chains::llm_chain::PromptChainCapability;
pub use context::conversation::{ConversationService, DefaultConversationService};
pub use context::tenant::{
    PlanLimits, PlanTier, SubscriptionStatus, TenantClaims, TenantContext, UserRole,
};
pub use identity::legacy::LegacyIdentityProvider;
pub use identity::zitadel::{ZitadelCacheStats, ZitadelProvider};
pub use identity::{
    AuthError, IdentityContext, IdentityManager, IdentityProvider, TenantCreated, TenantManager,
    TenantSummary,
};
#[cfg(feature = "local-embeddings")]
pub use indexing::LocalEmbeddingService;
pub use indexing::{EmbeddingModel, EmbeddingService, NoopEmbeddingService};
pub use memory::{ContextBuilder, ContextTruncator, OldestFirstTruncator};
pub use realtime::{InvalidationBus, InvalidationEvent, new_invalidation_bus};
pub use realtime::{RealtimeService, WorkspaceChangeEvent};
pub use store::onboarding::TenantOnboardingService;
pub use store::{
    CompletedPart, CredentialStore, DEFAULT_TENANT_ROOT_NAME, FinalizeResult, HttpMarkerClient,
    MarkerClient, NoopMarkerClient, OnboardingError, OnboardingOptions, QdrantVectorStore,
    RedbMetadataStore, RustFsContentStore, StorageCreds, StorageError, StorageLayout,
    StorageQuotaService, TenantKind, TenantStorage, TenantStorageFactory, TenantStorageMode,
    VirtualPath, WorkspaceStorage, build_root_store, extract_tenant_from_legacy_key,
    extract_virtual_path_from_key,
};

pub use llm::{
    CompletionProvider, LlmBinding, LlmChunk, LlmError, LlmRegistry, LlmRequest, LlmResponse,
    LlmStream, LlmUsage,
};

pub use model_catalog::{
    ModelCatalog, ModelError, ModelId, ModelSpec, ProviderKind, StaticModelCatalog,
    ToolRequirementReason, ToolRoutingDecision, estimate_input_tokens, token_estimate_exceeds_limit,
};

pub use workspace_ops::DeletePlanNode;
