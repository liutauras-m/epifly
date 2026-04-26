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

#[derive(Debug, serde::Serialize)]
pub struct ApiError {
    pub code: u16,
    pub message: String,
}

impl ApiError {
    pub fn new(code: u16, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
