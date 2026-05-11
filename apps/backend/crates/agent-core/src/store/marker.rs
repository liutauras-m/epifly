//! Marker PDF-to-markdown client.
//!
//! Thin `reqwest` wrapper around the Marker API (`MARKER_URL`).
//! Swappable via the trait for future WASM/browser-shell implementations.

use async_trait::async_trait;
use bytes::Bytes;
use tracing::instrument;

#[async_trait]
pub trait MarkerClient: Send + Sync + 'static {
    async fn pdf_to_markdown(&self, bytes: Bytes) -> anyhow::Result<String>;
}

// ── HTTP implementation ───────────────────────────────────────────────────────

pub struct HttpMarkerClient {
    client: reqwest::Client,
    url: String,
}

impl HttpMarkerClient {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            url: url.into(),
        }
    }

    pub fn from_env() -> Self {
        let url = std::env::var("MARKER_URL")
            .unwrap_or_else(|_| "http://marker-api:8080".into());
        Self::new(url)
    }
}

#[async_trait]
impl MarkerClient for HttpMarkerClient {
    #[instrument(skip(self, bytes), fields(bytes = bytes.len()))]
    async fn pdf_to_markdown(&self, bytes: Bytes) -> anyhow::Result<String> {
        let part = reqwest::multipart::Part::bytes(bytes.to_vec())
            .file_name("upload.pdf")
            .mime_str("application/pdf")?;
        let form = reqwest::multipart::Form::new().part("pdf", part);

        let resp = self
            .client
            .post(format!("{}/convert", self.url))
            .multipart(form)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Marker API error {status}: {body}");
        }

        let text = resp.text().await?;
        Ok(text)
    }
}

// ── Noop (for test mode) ─────────────────────────────────────────────────────

pub struct NoopMarkerClient;

#[async_trait]
impl MarkerClient for NoopMarkerClient {
    async fn pdf_to_markdown(&self, _bytes: Bytes) -> anyhow::Result<String> {
        Ok("[PDF conversion unavailable in test mode]".into())
    }
}
