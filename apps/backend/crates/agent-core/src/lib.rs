pub mod agent;
pub mod bridge;
pub mod capabilities;
pub mod chains;
pub mod context;
pub mod indexing;
pub mod llm;
pub mod memory;
pub mod prompt;
pub mod realtime;
pub mod store;
pub mod vector_store;

pub use agent::builder::{Agent, AgentBuilder};
pub use agent::hooks::{PermissionHook, TracingHook};
pub use agent::runtime::map_rig_error;
pub use bridge::ArtifactBridge;
pub use capabilities::admin::{
    AdminLimits, CapabilityAdmin, CapabilitySummary, CreateCapabilityRequest, TestInvokeRequest,
    TestInvokeResponse, UpdateCapabilityRequest, build_admin,
};
pub use capabilities::builtin_tool_card;
pub use capabilities::card::CapabilityCard;
pub use capabilities::discovery::CapabilityDiscovery;
pub use capabilities::namespace::NamespaceFilter;
pub use capabilities::provider::{BulkCapabilityFactory, CapabilityFactory};
pub use capabilities::providers::capability_spec::CapabilitySpecFactory;
pub use capabilities::registry::CapabilityRegistry;
pub use capabilities::semantic_router::{
    RouterMetrics, SemanticCapabilityRouter, SemanticRouterConfig,
};
pub use capabilities::store::{FilesystemStore, RegisteredToolState, RegisteredToolStore};
pub use capabilities::trace_replay::{
    TraceReplayCapability, TraceReplayFactory, WorkspaceNodeTraceSource,
};
pub use capabilities::validator::{
    RegisteredToolValidationError, RegisteredToolValidator, ValidationReport,
};
pub use chains::contract::{ContractData, ContractParty, ContractPipeline};
pub use chains::extraction::ExtractionPipeline;
pub use chains::invoice::{InvoiceData, InvoiceLineItem, InvoicePipeline};
pub use chains::llm_chain::PromptChainCapability;
pub use context::conversation::{ConversationService, DefaultConversationService};
pub use context::tenant::{PlanTier, TenantClaims, TenantContext, UserRole};
#[cfg(feature = "local-embeddings")]
pub use indexing::LocalEmbeddingService;
pub use indexing::{
    EmbeddingService, NoopEmbeddingService, OpenAiEmbeddingService, WorkspaceIndexer,
};
pub use memory::{ContextBuilder, ContextTruncator, OldestFirstTruncator};
pub use realtime::{RealtimeService, WorkspaceChangeEvent};
pub use store::{
    HttpMarkerClient, MarkerClient, NoopMarkerClient, QdrantVectorStore, RedbMetadataStore,
    RustFsContentStore,
};

pub use llm::{
    CompletionProvider, LlmBinding, LlmChunk, LlmError, LlmRegistry, LlmRequest, LlmResponse,
    LlmStream, LlmUsage,
};
