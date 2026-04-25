use super::manifest::CapabilityManifest;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct CapabilityCard {
    pub id: Uuid,
    pub manifest: CapabilityManifest,
    pub source_path: std::path::PathBuf,
    pub embedding_id: Option<String>,
}

impl CapabilityCard {
    pub fn new(manifest: CapabilityManifest, source_path: std::path::PathBuf) -> Self {
        Self {
            id: Uuid::new_v4(),
            manifest,
            source_path,
            embedding_id: None,
        }
    }
}
