pub mod embedding_service;
#[cfg(feature = "local-embeddings")]
pub mod local_embedding_service;

pub use embedding_service::{
    EMBEDDING_DIMS, EmbeddingModel, EmbeddingService, NoopEmbeddingService,
};
#[cfg(feature = "local-embeddings")]
pub use local_embedding_service::LocalEmbeddingService;
