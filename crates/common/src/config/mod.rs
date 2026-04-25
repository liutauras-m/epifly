use figment::{Figment, providers::{Env, Format, Toml}};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub qdrant: QdrantConfig,
    pub capabilities_dir: String,
    pub telemetry: TelemetryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QdrantConfig {
    pub url: String,
    pub collection: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConfig {
    pub otlp_endpoint: Option<String>,
    pub log_level: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig { host: "0.0.0.0".into(), port: 8080 },
            qdrant: QdrantConfig {
                url: "http://localhost:6334".into(),
                collection: "capabilities".into(),
            },
            capabilities_dir: "./capabilities".into(),
            telemetry: TelemetryConfig {
                otlp_endpoint: None,
                log_level: "info".into(),
            },
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
