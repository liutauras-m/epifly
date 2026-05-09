use serde::{Deserialize, Serialize};

/// Configuration block for a data-driven LLM chain tool (`kind = "chain"`).
/// Present only when the capability is implemented purely as an LLM prompt
/// (no hardcoded Rust provider needed).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmChainConfig {
    /// Alias or concrete model id (e.g. `"claude-opus-4-7"` or `"opus"`).
    pub model: String,
    #[serde(default)]
    pub system_prompt: Option<String>,
    /// `{{input.field}}` / `{{tenant.id}}` template for the user message.
    pub prompt_template: String,
    /// Whether the provider should pass an image from `input.image_path`.
    #[serde(default)]
    pub vision: bool,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    /// Optional JSON Schema that the LLM response must satisfy (validated after parsing).
    #[serde(default)]
    pub output_schema: Option<serde_json::Value>,
}

fn default_max_tokens() -> u32 {
    2048
}

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
    /// Primary namespace in dot-separated slug form, e.g. `"accounting.invoice"`.
    /// Optional — empty string means unnamespaced.
    #[serde(default)]
    pub namespace: Option<String>,
    /// Present when `kind = "chain"` and the capability is data-driven (no bespoke Rust).
    #[serde(default)]
    pub chain: Option<LlmChainConfig>,
    /// Empty = global (all tenants). Non-empty = only these tenant IDs see this capability.
    #[serde(default)]
    pub tenant_scope: Vec<String>,
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
    /// DB-backed, versioned prompt capability — no Rust rebuild required.
    #[serde(rename = "dynamic_prompt")]
    DynamicPrompt,
    /// External MCP service registered via JSON (no TOML file on disk).
    #[serde(rename = "remote_mcp")]
    RemoteMcp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

impl ToolManifest {
    pub fn from_toml(s: &str) -> common::error::Result<Self> {
        toml::from_str(s).map_err(|e| common::error::ConusAiError::Tool(e.to_string()))
    }

    pub fn from_file(path: &std::path::Path) -> common::error::Result<Self> {
        let s = std::fs::read_to_string(path).map_err(|e| {
            common::error::ConusAiError::Tool(format!("cannot read {:?}: {e}", path))
        })?;
        Self::from_toml(&s)
    }

    /// Returns the primary namespace or empty string if unset.
    pub fn namespace(&self) -> &str {
        self.namespace.as_deref().unwrap_or("")
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
