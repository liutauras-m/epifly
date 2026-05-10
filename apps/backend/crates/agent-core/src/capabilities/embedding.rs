use super::card::CapabilityCard;

/// Generates an embedding-friendly description for a tool.
/// Tool embedding helper — wraps a tool's embedding vector for storage/retrieval.
pub struct ToolEmbedding;

impl ToolEmbedding {
    pub fn describe(card: &CapabilityCard) -> String {
        card.manifest.embedding_text()
    }
}
