use figment::{
    Figment,
    providers::{Env, Format, Toml},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub storage: StorageConfig,
    pub capabilities_dir: String,
    pub telemetry: TelemetryConfig,
    pub llm: LlmConfig,
}

// ── LLM provider config ───────────────────────────────────────────────────────

/// Top-level `[llm]` section. Parsed by figment; overridable with
/// `CONUSAI_LLM__DEFAULT=...` and `CONUSAI_LLM__ALIASES__OPUS__MODEL=...`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// Global default alias (e.g. `"opus"`).
    pub default: String,
    /// Named alias → provider + model mapping.
    pub aliases: HashMap<String, LlmAliasConfig>,
    /// Per-provider connection config.
    pub providers: LlmProvidersConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmAliasConfig {
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LlmProvidersConfig {
    pub anthropic: Option<AnthropicProviderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicProviderConfig {
    /// Env var name that holds the API key (default: `"ANTHROPIC_API_KEY"`).
    #[serde(default = "default_anthropic_key_env")]
    pub api_key_env: String,
    #[serde(default = "default_anthropic_base_url")]
    pub base_url: String,
    #[serde(default = "default_anthropic_api_version")]
    pub api_version: String,
}

fn default_anthropic_key_env() -> String {
    "ANTHROPIC_API_KEY".into()
}
fn default_anthropic_base_url() -> String {
    "https://api.anthropic.com".into()
}
fn default_anthropic_api_version() -> String {
    "2023-06-01".into()
}

impl Default for AnthropicProviderConfig {
    fn default() -> Self {
        Self {
            api_key_env: default_anthropic_key_env(),
            base_url: default_anthropic_base_url(),
            api_version: default_anthropic_api_version(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// redb database file path. Read from REDB_PATH env (via figment CONUSAI_ prefix).
    pub redb_path: String,
    /// Qdrant gRPC endpoint URL. Read from QDRANT_URL.
    pub qdrant_url: String,
    /// S3-compatible object store endpoint. Read from S3_ENDPOINT.
    pub s3_endpoint: String,
    /// S3 bucket name. Read from S3_BUCKET.
    pub s3_bucket: String,
    /// Marker PDF-to-markdown service URL. Read from MARKER_URL.
    pub marker_url: Option<String>,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            redb_path: "/data/conusai.redb".into(),
            qdrant_url: "http://localhost:6334".into(),
            s3_endpoint: "http://localhost:9000".into(),
            s3_bucket: "workspace".into(),
            marker_url: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConfig {
    pub otlp_endpoint: Option<String>,
    pub log_level: String,
}

impl Default for LlmConfig {
    fn default() -> Self {
        let mut aliases = HashMap::new();
        aliases.insert(
            "opus".into(),
            LlmAliasConfig {
                provider: "anthropic".into(),
                model: "claude-opus-4-7".into(),
            },
        );
        aliases.insert(
            "haiku".into(),
            LlmAliasConfig {
                provider: "anthropic".into(),
                model: "claude-haiku-4-5-20251001".into(),
            },
        );
        Self {
            default: "opus".into(),
            aliases,
            providers: LlmProvidersConfig {
                anthropic: Some(AnthropicProviderConfig::default()),
            },
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "0.0.0.0".into(),
                port: 8080,
            },
            storage: StorageConfig::default(),
            capabilities_dir: "./capabilities".into(),
            telemetry: TelemetryConfig {
                otlp_endpoint: None,
                log_level: "info".into(),
            },
            llm: LlmConfig::default(),
        }
    }
}

impl AppConfig {
    pub fn load() -> crate::error::Result<Self> {
        let config = Figment::new()
            .merge(Toml::file("config.toml"))
            .merge(Env::prefixed("CONUSAI_"))
            .extract()
            .map_err(|e| crate::error::ConusAiError::Config(e.to_string()))?;
        Ok(config)
    }
}
