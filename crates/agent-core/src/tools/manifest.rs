use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub kind: ToolKind,
    pub tools: Vec<ToolDef>,
    #[serde(default)]
    pub config: serde_json::Value,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ToolKind {
    Mcp,
    Wasm,
    Chain,
    Docker,
    /// Built-in in-process tools (filesystem, cargo runner). Not loaded from YAML.
    Native,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

impl ToolManifest {
    pub fn from_yaml(s: &str) -> common::error::Result<Self> {
        serde_yaml::from_str(s).map_err(|e| common::error::ConusAiError::Tool(e.to_string()))
    }

    pub fn from_file(path: &std::path::Path) -> common::error::Result<Self> {
        let s = std::fs::read_to_string(path).map_err(|e| {
            common::error::ConusAiError::Tool(format!("cannot read {:?}: {e}", path))
        })?;
        Self::from_yaml(&s)
    }

    pub fn embedding_text(&self) -> String {
        let tools = self
            .tools
            .iter()
            .map(|t| format!("  - {}: {}", t.name, t.description))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "Tool: {}\nDescription: {}\nKind: {:?}\nTools:\n{}",
            self.name, self.description, self.kind, tools
        )
    }
}
