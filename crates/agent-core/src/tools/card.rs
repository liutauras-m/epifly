use super::manifest::ToolManifest;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ToolCard {
    pub id: Uuid,
    pub manifest: ToolManifest,
    pub source_path: std::path::PathBuf,
    pub embedding_id: Option<String>,
}

impl ToolCard {
    pub fn new(manifest: ToolManifest, source_path: std::path::PathBuf) -> Self {
        Self {
            id: Uuid::new_v4(),
            manifest,
            source_path,
            embedding_id: None,
        }
    }
}
