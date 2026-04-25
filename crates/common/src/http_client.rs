use reqwest::Client;
use std::time::Duration;

pub fn build_client() -> crate::error::Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(60))
        .user_agent("conusai-platform/0.1")
        .build()
        .map_err(|e| crate::error::ConusAiError::Other(e.into()))
}
