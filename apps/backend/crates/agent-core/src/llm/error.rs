use thiserror::Error;

#[derive(Error, Debug)]
pub enum LlmError {
    #[error("provider '{provider}' error: {message}")]
    Provider { provider: &'static str, message: String },

    #[error("rig completion error: {0}")]
    RigCompletion(#[from] rig::completion::CompletionError),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("unknown alias '{alias}'")]
    UnknownAlias { alias: String },

    #[error("provider '{0}' not registered")]
    UnknownProvider(String),

    #[error("tenant override invalid: {0}")]
    TenantOverride(String),
}
