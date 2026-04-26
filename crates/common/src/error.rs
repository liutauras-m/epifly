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

    #[error("mcp error: {0}")]
    Mcp(String),

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
