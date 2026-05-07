pub mod agent;
pub mod chains;
pub mod context;
pub mod indexing;
pub mod llm;
pub mod memory;
pub mod prompt;
pub mod realtime;
pub mod tools;
pub mod vector_store;

pub use agent::builder::{Agent, AgentBuilder};
pub use agent::hooks::{PermissionHook, TracingHook};
pub use agent::runtime::map_rig_error;
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
pub use memory::{
    ContextBuilder, ContextTruncator, MinioWorkspaceContent, OldestFirstTruncator,
    PostgresAuditStore, PostgresThreadStore, PostgresWorkspaceStore,
};
pub use realtime::{RealtimeService, WorkspaceChangeEvent};
pub use tools::admin::{
    AdminLimits, CapabilityAdmin, CapabilitySummary, CreateCapabilityRequest, TestInvokeRequest,
    TestInvokeResponse, UpdateCapabilityRequest, build_admin,
};
pub use tools::builtin_tool_card;
pub use tools::card::CapabilityCard;
pub use tools::discovery::ToolDiscovery;
pub use tools::namespace::NamespaceFilter;
pub use tools::provider::{BulkCapabilityFactory, CapabilityFactory};
pub use tools::providers::capability_spec::CapabilitySpecFactory;
pub use tools::registry::ToolRegistry;
pub use tools::semantic_router::{RouterMetrics, SemanticCapabilityRouter, SemanticRouterConfig};
pub use tools::store::{FilesystemStore, RegisteredToolState, RegisteredToolStore};
pub use tools::validator::{
    RegisteredToolValidationError, RegisteredToolValidator, ValidationReport,
};
pub use vector_store::PgVectorStore;

pub use llm::{
    CompletionProvider, LlmBinding, LlmChunk, LlmError, LlmRegistry, LlmRequest, LlmResponse,
    LlmStream, LlmUsage,
};
