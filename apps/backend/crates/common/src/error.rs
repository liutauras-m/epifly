use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

pub type Result<T, E = ConusAiError> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum ConusAiError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("tool error: {0}")]
    Tool(String),

    #[error("wasm error: {0}")]
    Wasm(String),

    /// Wraps `wasmtime::Error` — `#[from]` not used to avoid a transitive dep in `common`.
    /// Convert with `ConusAiError::WasmRuntime(e.to_string())` at the call site.
    #[error("wasm runtime: {0}")]
    WasmRuntime(String),

    #[error("mcp error: {0}")]
    Mcp(String),

    /// Wraps rig completion errors — rig's error types are not `'static + Send` across
    /// all 0.9.x point releases, so we capture the message as a `String`.
    /// Convert with `ConusAiError::Rig(e.to_string())` at the call site.
    #[error("rig: {0}")]
    Rig(String),

    /// Wraps Qdrant/reqwest errors from the vector-store helpers.
    /// Convert with `ConusAiError::Qdrant(e.to_string())` at the call site.
    #[error("qdrant: {0}")]
    Qdrant(String),

    #[error("storage error: {0}")]
    Storage(String),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("api error {status}: {message}")]
    Api { status: u16, message: String },

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

// ── HTTP Error envelope ────────────────────────────────────────────────────────

/// Structured error discriminants returned in every HTTP error response.
#[derive(Debug, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ApiErrorKind {
    Authentication { message: String },
    RateLimit { message: String, retry_after: Option<u64> },
    NotFound { resource: String },
    Validation { field: String, message: String },
    Agent { message: String },
    Internal { message: String, request_id: Option<String> },
}

/// `{"error": {...}}` — the single error envelope shape for all HTTP responses.
#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct ErrorEnvelope {
    pub error: ApiErrorBody,
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct ApiErrorBody {
    pub r#type: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

/// Axum-compatible HTTP error — carries its own status code and serializes as `ErrorEnvelope`.
#[derive(Debug)]
pub struct HttpError {
    pub status: StatusCode,
    pub body: ApiErrorBody,
}

impl HttpError {
    pub fn auth(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            body: ApiErrorBody {
                r#type: "authentication",
                message: msg.into(),
                field: None,
                resource: None,
                retry_after: None,
                request_id: None,
            },
        }
    }

    pub fn rate_limit(retry_after: Option<u64>) -> Self {
        Self {
            status: StatusCode::TOO_MANY_REQUESTS,
            body: ApiErrorBody {
                r#type: "rate_limit",
                message: "rate limit exceeded".into(),
                field: None,
                resource: None,
                retry_after,
                request_id: None,
            },
        }
    }

    pub fn not_found(resource: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            body: ApiErrorBody {
                r#type: "not_found",
                message: format!("not found: {}", resource.into()),
                field: None,
                resource: None,
                retry_after: None,
                request_id: None,
            },
        }
    }

    pub fn validation(field: impl Into<String>, message: impl Into<String>) -> Self {
        let f = field.into();
        let m = message.into();
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            body: ApiErrorBody {
                r#type: "validation",
                message: m,
                field: Some(f),
                resource: None,
                retry_after: None,
                request_id: None,
            },
        }
    }

    pub fn agent(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            body: ApiErrorBody {
                r#type: "agent",
                message: msg.into(),
                field: None,
                resource: None,
                retry_after: None,
                request_id: None,
            },
        }
    }

    pub fn internal(msg: impl Into<String>, request_id: Option<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            body: ApiErrorBody {
                r#type: "internal",
                message: msg.into(),
                field: None,
                resource: None,
                retry_after: None,
                request_id,
            },
        }
    }

    pub fn with_request_id(mut self, id: impl Into<String>) -> Self {
        self.body.request_id = Some(id.into());
        self
    }
}

impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        let status = self.status;
        (status, Json(ErrorEnvelope { error: self.body })).into_response()
    }
}
