use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub kind: CapabilityKind,
    pub tools: Vec<ToolDef>,
    #[serde(default)]
    pub config: serde_json::Value,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CapabilityKind {
    Mcp,
    Wasm,
    Pipeline,
    Docker,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

impl CapabilityManifest {
    pub fn from_yaml(s: &str) -> common::error::Result<Self> {
        serde_yaml::from_str(s)
            .map_err(|e| common::error::ConusAiError::Capability(e.to_string()))
    }

    pub fn from_file(path: &std::path::Path) -> common::error::Result<Self> {
        let s = std::fs::read_to_string(path)
            .map_err(|e| common::error::ConusAiError::Capability(
                format!("cannot read {:?}: {e}", path)
            ))?;
        Self::from_yaml(&s)
    }

    pub fn embedding_text(&self) -> String {
        let tools = self.tools.iter()
            .map(|t| format!("  - {}: {}", t.name, t.description))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "Capability: {}\nDescription: {}\nKind: {:?}\nTools:\n{}",
            self.name, self.description, self.kind, tools
        )
    }
}
