pub mod coco_indexer;
pub mod embedding_service;
#[cfg(feature = "local-embeddings")]
pub mod local_embedding_service;

pub use coco_indexer::WorkspaceIndexer;
pub use embedding_service::{
    EMBEDDING_DIMS, EMBEDDING_MODEL, EmbeddingService, NoopEmbeddingService, OpenAiEmbeddingService,
};
#[cfg(feature = "local-embeddings")]
pub use local_embedding_service::LocalEmbeddingService;
