use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalConfig {
    pub suite: String,
    pub model: String,
    pub dataset_path: std::path::PathBuf,
}
