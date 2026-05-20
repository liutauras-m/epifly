use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: &str = "2.0";

/// Configuration block for a data-driven LLM chain tool (`kind = "chain"`).
/// Present only when the capability is implemented purely as an LLM prompt
/// (no hardcoded Rust provider needed).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmChainConfig {
    /// Alias or concrete model id (e.g. `"claude-opus-4-7"` or `"smart"`).
    pub model: String,
    #[serde(default)]
    pub system_prompt: Option<String>,
    /// `{{input.field}}` / `{{tenant.id}}` template for the user message.
    pub prompt_template: String,
    /// When true, the executor reads `input.image_path`, downloads/reads it,
    /// and sends base64-encoded image bytes as vision content alongside the text prompt.
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

/// Declares an acceptable attachment MIME type with optional size limit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptSpec {
    /// MIME glob, e.g. `"application/pdf"`, `"image/*"`, `"*"`.
    pub mime: String,
    /// Maximum file size in megabytes (absent = no limit).
    #[serde(default)]
    pub max_size_mb: Option<u32>,
}

/// Rough cost tier for planner ranking. All fields optional.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CostHint {
    pub tokens: Option<u64>,
    pub dollars: Option<f32>,
    pub latency_ms: Option<u64>,
}

impl CostHint {
    /// Coarse bucket string for embedding enrichment: `"cheap"` / `"standard"` / `"premium"`.
    pub fn bucket(&self) -> &'static str {
        match self.dollars {
            Some(d) if d < 0.01 => "cheap",
            Some(d) if d < 0.10 => "standard",
            Some(_) => "premium",
            None => match self.tokens {
                Some(t) if t < 1_000 => "cheap",
                Some(t) if t < 10_000 => "standard",
                Some(_) => "premium",
                None => "standard",
            },
        }
    }
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
    /// Primary namespace in dot-separated slug form, e.g. `"extract.fields.invoice"`.
    /// Optional — empty string means unnamespaced.
    #[serde(default)]
    pub namespace: Option<String>,
    /// Present when `kind = "chain"` and the capability is data-driven (no bespoke Rust).
    #[serde(default)]
    pub chain: Option<LlmChainConfig>,
    /// Empty = global (all tenants). Non-empty = only these tenant IDs see this capability.
    #[serde(default)]
    pub tenant_scope: Vec<String>,
    /// Set to false in capability.toml to disable without deleting the directory.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Extra terms/phrases injected into the embedding text only — not shown to Claude.
    /// Use for synonyms, example queries, and routing hints like "use when user mentions invoice".
    #[serde(default)]
    pub search_keywords: Vec<String>,

    // ── v2 fields (all optional for backwards compatibility) ──────────────────

    /// Schema version: `"1.0"` (legacy) or `"2.0"` (current). Loader accepts both.
    #[serde(default = "default_schema_version")]
    pub schema_version: String,
    /// Taxonomy root from `docs/capabilities/taxonomy.md`, e.g. `"extract"`, `"storage"`.
    #[serde(default)]
    pub category: Option<String>,
    /// MIME types this capability can process as attachments (router post-filter).
    #[serde(default)]
    pub accepts: Vec<AcceptSpec>,
    /// MIME types this capability may emit as output artifacts.
    #[serde(default)]
    pub emits: Vec<String>,
    /// Whether this capability is safe to retry or run in parallel (default: true).
    #[serde(default = "default_true")]
    pub idempotent: bool,
    /// Rough cost estimate for planner ranking.
    #[serde(default)]
    pub cost_hint: Option<CostHint>,
    /// Capability names that must be registered for this capability to function.
    /// Router warns at load time if any are missing.
    #[serde(default)]
    pub requires: Vec<String>,
}

fn default_schema_version() -> String {
    "1.0".to_string()
}

fn default_true() -> bool {
    true
}

fn default_enabled() -> bool {
    true
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
        let mut parts = vec![
            format!("Capability: {}", self.name),
            format!("Description: {}", self.description),
            format!("Tags: {}", self.tags.join(", ")),
            format!("Tools:\n{tools}"),
        ];
        if !self.search_keywords.is_empty() {
            parts.push(format!("Keywords: {}", self.search_keywords.join(", ")));
        }
        // Phase 2.1a: enrich with CATEGORY and MIME tokens for ANN recall.
        if let Some(cat) = &self.category {
            parts.push(format!("CATEGORY:{cat}"));
        }
        for accept in &self.accepts {
            parts.push(format!("MIME:{}", accept.mime));
        }
        for emit in &self.emits {
            parts.push(format!("EMITS:{emit}"));
        }
        if let Some(hint) = &self.cost_hint {
            parts.push(format!("COST:{}", hint.bucket()));
        }
        parts.join("\n")
    }
}
