use super::card::CapabilityCard;

/// Generates an embedding-friendly description for a capability.
/// Full Qdrant integration wired in Phase 6.
pub struct ToolEmbedding;

impl ToolEmbedding {
    pub fn describe(card: &CapabilityCard) -> String {
        card.manifest.embedding_text()
    }
}
