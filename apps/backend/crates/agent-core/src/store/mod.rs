pub mod creds;
pub mod marker;
pub mod onboarding;
pub mod qdrant_vector;
pub mod quota;
pub mod redb_metadata;
pub mod rustfs_content;
pub mod tenant_storage;

pub use creds::{CredentialStore, StorageCreds};
pub use marker::{HttpMarkerClient, MarkerClient, NoopMarkerClient};
pub use onboarding::{OnboardingError, OnboardingOptions, TenantKind, TenantOnboardingService};
pub use qdrant_vector::{CapabilityHit, ContentHit, QdrantConfig, QdrantVectorStore};
pub use quota::StorageQuotaService;
pub use redb_metadata::RedbMetadataStore;
pub use rustfs_content::RustFsContentStore;
pub use tenant_storage::{
    CompletedPart, DEFAULT_TENANT_ROOT_NAME, FinalizeResult, StorageError, StorageLayout,
    TenantStorage, TenantStorageFactory, VirtualPath, WorkspaceStorage,
    extract_tenant_from_legacy_key, extract_virtual_path_from_key,
};
