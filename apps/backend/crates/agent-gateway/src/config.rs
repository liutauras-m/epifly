//! The gateway's environment contract, validated in full at every start.
//!
//! [`GatewayConfig::from_env`] reads every variable the gateway uses and fails
//! with one error naming every missing or invalid variable. There are no
//! development fallbacks: secrets and service locations have no defaults.

use crate::auth::ApiKeyEntry;
use agent_core::capabilities::mcp_allowlist::{self, McpAllowlist};
use agent_core::identity::zitadel::IntrospectionClient;
use agent_core::{EmbeddingModel, QdrantConfig, ZitadelConfig};
use base64::Engine as _;
use billing_core::lago::LagoConfig;
use common::env::{EnvContract, EnvContractError};
use rustfs_admin::{BootstrapConfig, NotificationTarget, S3Config};
use std::path::PathBuf;
use std::time::Duration;

/// Origins of the native (Tauri) shell, which calls the gateway directly with a
/// bearer token. Always allowed in addition to `WEB_ORIGIN`.
pub const NATIVE_SHELL_ORIGINS: [&str; 2] = ["tauri://localhost", "http://tauri.localhost"];

/// Minimum length of `PLATFORM_ADMIN_TOKEN`.
pub const MIN_PLATFORM_TOKEN_LEN: usize = 32;

pub struct EmbeddingConfig {
    pub model: EmbeddingModel,
    pub cache_dir: PathBuf,
    pub max_batch: usize,
}

pub struct GatewayConfig {
    pub port: u16,
    pub web_origins: Vec<String>,
    pub platform_admin_token: String,
    pub zitadel: ZitadelConfig,
    pub api_keys: Vec<ApiKeyEntry>,
    pub anthropic_api_key: String,
    pub s3: S3Config,
    pub iam_enc_key: [u8; 32],
    pub qdrant: QdrantConfig,
    pub redb_path: PathBuf,
    pub capabilities_dir: PathBuf,
    pub embedding: EmbeddingConfig,
    pub semantic_top_k: usize,
    pub mcp_allowlist: McpAllowlist,
    pub lago: Option<LagoConfig>,
    pub bootstrap: BootstrapConfig,
    /// HMAC secret for `POST /internal/rustfs/events`; the route exists only when set.
    pub rustfs_webhook_secret: Option<String>,
    pub presign_ttl: Duration,
    pub public_base_url: Option<String>,
    pub storage_quotas: bool,
}

impl GatewayConfig {
    pub fn from_env() -> Result<Self, EnvContractError> {
        let mut c = EnvContract::new();

        let port = c.parse_or("PORT", 8080u16);

        let mut web_origins: Vec<String> = c
            .required("WEB_ORIGIN")
            .split(',')
            .map(|o| o.trim().trim_end_matches('/').to_string())
            .filter(|o| !o.is_empty())
            .collect();
        for o in &web_origins {
            match url::Url::parse(o) {
                Ok(u) if matches!(u.scheme(), "http" | "https") && u.path() == "/" => {}
                _ => c.invalid("WEB_ORIGIN", format!("{o} is not an http(s) origin")),
            }
        }
        for o in NATIVE_SHELL_ORIGINS {
            if !web_origins.iter().any(|w| w == o) {
                web_origins.push(o.to_string());
            }
        }

        let platform_admin_token = c.required("PLATFORM_ADMIN_TOKEN");
        if !platform_admin_token.is_empty() && platform_admin_token.len() < MIN_PLATFORM_TOKEN_LEN {
            c.invalid(
                "PLATFORM_ADMIN_TOKEN",
                format!("must be at least {MIN_PLATFORM_TOKEN_LEN} characters"),
            );
        }

        let introspection = match (
            c.optional("ZITADEL_INTROSPECTION_CLIENT_ID"),
            c.optional("ZITADEL_INTROSPECTION_CLIENT_SECRET"),
        ) {
            (Some(client_id), Some(client_secret)) => Some(IntrospectionClient {
                client_id,
                client_secret,
            }),
            (None, None) => None,
            _ => {
                c.invalid(
                    "ZITADEL_INTROSPECTION_CLIENT_ID",
                    "set both ZITADEL_INTROSPECTION_CLIENT_ID and ZITADEL_INTROSPECTION_CLIENT_SECRET, or neither",
                );
                None
            }
        };
        let zitadel = ZitadelConfig {
            issuer: c
                .required("ZITADEL_ISSUER")
                .trim_end_matches('/')
                .to_string(),
            audience: c.required("ZITADEL_AUDIENCE"),
            introspection,
        };
        if !zitadel.issuer.is_empty()
            && !zitadel.audience.is_empty()
            && let Err(e) = zitadel.validate()
        {
            c.invalid("ZITADEL_ISSUER", e);
        }

        let api_keys = match c.optional("API_KEYS") {
            None => Vec::new(),
            Some(raw) => match crate::auth::parse_api_keys(&raw) {
                Ok(keys) => keys,
                Err(e) => {
                    c.invalid("API_KEYS", e);
                    Vec::new()
                }
            },
        };

        let anthropic_api_key = c.required("ANTHROPIC_API_KEY");

        let s3 = S3Config {
            endpoint: c.required("AWS_ENDPOINT_URL"),
            access_key_id: c.required("AWS_ACCESS_KEY_ID"),
            secret_access_key: c.required("AWS_SECRET_ACCESS_KEY"),
            bucket: c.required("S3_BUCKET"),
        };
        if !s3.endpoint.is_empty() && url::Url::parse(&s3.endpoint).is_err() {
            c.invalid("AWS_ENDPOINT_URL", "not a URL");
        }

        let mut iam_enc_key = [0u8; 32];
        let raw_key = c.required("RUSTFS_IAM_ENC_KEY");
        if !raw_key.is_empty() {
            match base64::engine::general_purpose::STANDARD.decode(raw_key.trim()) {
                Ok(bytes) if bytes.len() == 32 => iam_enc_key.copy_from_slice(&bytes),
                _ => c.invalid("RUSTFS_IAM_ENC_KEY", "must be base64 of exactly 32 bytes"),
            }
        }

        let qdrant = QdrantConfig {
            grpc_url: c.required("QDRANT_GRPC_URL"),
            api_key: c.required("QDRANT_API_KEY"),
        };
        if !qdrant.grpc_url.is_empty() && url::Url::parse(&qdrant.grpc_url).is_err() {
            c.invalid("QDRANT_GRPC_URL", "not a URL");
        }

        let redb_path = PathBuf::from(
            c.optional("REDB_PATH")
                .unwrap_or_else(|| "/data/conusai.redb".into()),
        );
        let capabilities_dir = PathBuf::from(c.required("CONUSAI_CAPABILITIES_DIR"));

        let model_name = c
            .optional("EMBEDDING_LOCAL_MODEL")
            .unwrap_or_else(|| "multilingual-e5-large".into());
        let model = EmbeddingModel::from_name(&model_name).unwrap_or_else(|| {
            c.invalid(
                "EMBEDDING_LOCAL_MODEL",
                format!("unknown model {model_name}"),
            );
            EmbeddingModel::MultilingualE5Large
        });
        let embedding = EmbeddingConfig {
            model,
            cache_dir: PathBuf::from(c.required("EMBEDDING_CACHE_DIR")),
            max_batch: c.parse_or("EMBEDDING_MAX_BATCH", 256usize),
        };

        let semantic_top_k = c.parse_or("SEMANTIC_ROUTER_TOP_K", 20usize);
        let mcp_allowlist =
            McpAllowlist::parse(&c.optional(mcp_allowlist::ENV_VAR).unwrap_or_default());

        let lago = c.optional("LAGO_API_KEY").map(|api_key| LagoConfig {
            api_url: c.required("LAGO_API_URL").trim_end_matches('/').to_string(),
            api_key,
            webhook_secret: c.required("LAGO_WEBHOOK_SECRET"),
        });

        let notification = match (
            c.optional("RUSTFS_NOTIFICATION_WEBHOOK_URL"),
            c.optional("RUSTFS_WEBHOOK_SECRET"),
        ) {
            (Some(webhook_url), Some(_)) => Some(NotificationTarget { webhook_url }),
            (None, None) => None,
            _ => {
                c.invalid(
                    "RUSTFS_NOTIFICATION_WEBHOOK_URL",
                    "set both RUSTFS_NOTIFICATION_WEBHOOK_URL and RUSTFS_WEBHOOK_SECRET, or neither",
                );
                None
            }
        };
        let rustfs_webhook_secret = notification
            .as_ref()
            .and_then(|_| c.optional("RUSTFS_WEBHOOK_SECRET"));
        let bootstrap = BootstrapConfig {
            enabled: c.switch("RUSTFS_BOOTSTRAP", true),
            encryption: c.switch("RUSTFS_SSE", true),
            versioning: c.switch("RUSTFS_VERSIONING", true),
            lifecycle: true,
            cors_origins: web_origins.clone(),
            notification,
        };

        let presign_ttl =
            Duration::from_secs(c.parse_or("RUSTFS_PRESIGN_TTL_SECS", 900u64).min(3600));
        let public_base_url = c.optional("RUSTFS_PUBLIC_BASE_URL");
        let storage_quotas = c.switch("RUSTFS_QUOTAS", true);

        // Optional tunables read by other modules: validate their format here so a
        // typo fails the start instead of silently falling back to a default.
        let _: u32 = c.parse_or("CONUSAI_MAX_TOOLS_PER_TURN", 0);
        let _: u32 = c.parse_or("CONUSAI_MAX_INVOKES_PER_TURN", 0);
        let _: f64 = c.parse_or("CONUSAI_ROUTER_MIN_CONFIDENCE", 0.0);
        let _: i64 = c.parse_or("AUDIT_RETENTION_DAYS", 90);
        let _: u64 = c.parse_or("RUSTFS_KEY_ROTATION_DAYS", 90);
        if let Some(path) = c.optional("CONUSAI_PLAN_CATALOG_PATH")
            && !std::path::Path::new(&path).is_file()
        {
            c.invalid("CONUSAI_PLAN_CATALOG_PATH", format!("{path} is not a file"));
        }

        c.finish()?;
        Ok(Self {
            port,
            web_origins,
            platform_admin_token,
            zitadel,
            api_keys,
            anthropic_api_key,
            s3,
            iam_enc_key,
            qdrant,
            redb_path,
            capabilities_dir,
            embedding,
            semantic_top_k,
            mcp_allowlist,
            lago,
            bootstrap,
            rustfs_webhook_secret,
            presign_ttl,
            public_base_url,
            storage_quotas,
        })
    }
}
