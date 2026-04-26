use super::card::ToolCard;

/// Generates an embedding-friendly description for a tool.
/// Full Qdrant integration wired in Phase 6.
pub struct ToolEmbedding;

impl ToolEmbedding {
    pub fn describe(card: &ToolCard) -> String {
        card.manifest.embedding_text()
    }
}
