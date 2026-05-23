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
///
/// In TOML manifests this accepts either a bare MIME string —
/// `accepts = ["application/json"]` — or an object with a size cap —
/// `accepts = [{ mime = "application/pdf", max_size_mb = 20 }]`.
#[derive(Debug, Clone, Serialize)]
pub struct AcceptSpec {
    /// MIME glob, e.g. `"application/pdf"`, `"image/*"`, `"*"`.
    pub mime: String,
    /// Maximum file size in megabytes (absent = no limit).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_size_mb: Option<u32>,
}

impl<'de> Deserialize<'de> for AcceptSpec {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Form {
            Bare(String),
            Full {
                mime: String,
                #[serde(default)]
                max_size_mb: Option<u32>,
            },
        }
        Ok(match Form::deserialize(d)? {
            Form::Bare(mime) => AcceptSpec {
                mime,
                max_size_mb: None,
            },
            Form::Full { mime, max_size_mb } => AcceptSpec { mime, max_size_mb },
        })
    }
}

/// Rough cost tier for planner ranking. All fields optional.
///
/// In TOML manifests this accepts either a bare bucket label —
/// `cost_hint = "low"` (also `"cheap"`, `"medium"`, `"standard"`, `"high"`, `"premium"`)
/// — or a full object — `cost_hint = { dollars = 0.05, latency_ms = 4000 }`.
/// Bare labels are mapped to representative `dollars` values so the existing
/// `bucket()` logic remains the single source of truth.
#[derive(Debug, Clone, Serialize, Default)]
pub struct CostHint {
    pub tokens: Option<u64>,
    pub dollars: Option<f32>,
    pub latency_ms: Option<u64>,
}

impl<'de> Deserialize<'de> for CostHint {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Form {
            Bare(String),
            Full {
                #[serde(default)]
                tokens: Option<u64>,
                #[serde(default)]
                dollars: Option<f32>,
                #[serde(default)]
                latency_ms: Option<u64>,
            },
        }
        Ok(match Form::deserialize(d)? {
            Form::Bare(label) => {
                // Bucket label → representative dollars so `bucket()` still works.
                let dollars = match label.to_ascii_lowercase().as_str() {
                    "low" | "cheap" => Some(0.005),
                    "medium" | "standard" => Some(0.05),
                    "high" | "premium" => Some(0.50),
                    _ => {
                        return Err(serde::de::Error::custom(format!(
                            "cost_hint string must be one of low/cheap/medium/standard/high/premium, got {label:?}"
                        )));
                    }
                };
                CostHint {
                    tokens: None,
                    dollars,
                    latency_ms: None,
                }
            }
            Form::Full {
                tokens,
                dollars,
                latency_ms,
            } => CostHint {
                tokens,
                dollars,
                latency_ms,
            },
        })
    }
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
    /// Per-tool keywords for the lexical capability prefilter (PR 2.B.2).
    /// Checked in `lexical_capability_hints()` with Unicode word-boundary matching.
    /// Default empty; fully backward-compatible — old manifests behave as today.
    #[serde(default)]
    pub search_keywords: Vec<String>,
    /// Optional: name of the input field that holds a workspace-relative path to
    /// read before invoking the tool (PR 2.D — read-before-write pattern).
    ///
    /// When set, the gateway reads the file at `input[field]` from
    /// `WorkspaceContentStore` and injects its text as `input._current_content`
    /// before the tool runs. If the file does not exist, injects
    /// `_current_content: null` and `_is_new_file: true` so the chain prompt can
    /// branch cleanly for the "create from scratch" case.
    ///
    /// Example (in TOML manifest):
    /// ```toml
    /// [[tools]]
    /// name              = "add_dependency"
    /// read_before_write = "manifest_path"
    /// ```
    #[serde(default)]
    pub read_before_write: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::*;

    // Both bare-string and full-object forms must parse and route to the same
    // bucket so existing manifests don't have to migrate.
    #[test]
    fn cost_hint_accepts_bare_bucket_labels() {
        #[derive(Deserialize)]
        struct W {
            cost_hint: CostHint,
        }
        let low: W = toml::from_str(r#"cost_hint = "low""#).unwrap();
        assert_eq!(low.cost_hint.bucket(), "cheap");
        let med: W = toml::from_str(r#"cost_hint = "medium""#).unwrap();
        assert_eq!(med.cost_hint.bucket(), "standard");
        let high: W = toml::from_str(r#"cost_hint = "high""#).unwrap();
        assert_eq!(high.cost_hint.bucket(), "premium");
        // Aliases
        let cheap: W = toml::from_str(r#"cost_hint = "cheap""#).unwrap();
        assert_eq!(cheap.cost_hint.bucket(), "cheap");
        let premium: W = toml::from_str(r#"cost_hint = "premium""#).unwrap();
        assert_eq!(premium.cost_hint.bucket(), "premium");
    }

    #[test]
    fn cost_hint_accepts_full_object() {
        #[derive(Deserialize)]
        struct W {
            cost_hint: CostHint,
        }
        let w: W = toml::from_str(r#"cost_hint = { dollars = 0.05, latency_ms = 4000 }"#).unwrap();
        assert_eq!(w.cost_hint.dollars, Some(0.05));
        assert_eq!(w.cost_hint.latency_ms, Some(4000));
        assert_eq!(w.cost_hint.bucket(), "standard");
    }

    #[test]
    fn cost_hint_rejects_unknown_label() {
        #[derive(Debug, Deserialize)]
        struct W {
            #[allow(dead_code)]
            cost_hint: CostHint,
        }
        let err = toml::from_str::<W>(r#"cost_hint = "exorbitant""#).unwrap_err();
        assert!(
            err.to_string().contains("cost_hint string must be one of"),
            "got: {err}"
        );
    }

    // Mirrors the AcceptSpec untagged-enum fix — accepts both bare strings
    // and full objects in TOML manifests.
    #[test]
    fn accept_spec_accepts_bare_and_object_forms() {
        #[derive(Deserialize)]
        struct W {
            accepts: Vec<AcceptSpec>,
        }
        let w: W = toml::from_str(
            r#"
            accepts = [
              "application/pdf",
              { mime = "image/*", max_size_mb = 20 },
            ]
            "#,
        )
        .unwrap();
        assert_eq!(w.accepts[0].mime, "application/pdf");
        assert_eq!(w.accepts[0].max_size_mb, None);
        assert_eq!(w.accepts[1].mime, "image/*");
        assert_eq!(w.accepts[1].max_size_mb, Some(20));
    }
}
