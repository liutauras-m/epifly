use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A file artifact produced by a tool invocation.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Artifact {
    /// Filename including extension, e.g. "transcript_2026.txt".
    pub name: String,
    /// MIME type, e.g. "text/plain", "application/pdf".
    pub mime_type: String,
    /// Base64-encoded content for small files (< 1 MiB). Mutually exclusive with `source_url`.
    #[serde(default)]
    pub data: Option<String>,
    /// Pre-signed or direct URL for large files. Mutually exclusive with `data`.
    #[serde(default)]
    pub source_url: Option<String>,
    /// Domain-specific metadata (e.g. `{"duration": 184.5, "language": "en"}`).
    #[serde(default)]
    pub metadata: Value,
}

/// Canonical tool output envelope.
/// All `CapabilityProvider::invoke()` implementations may return this as their JSON value.
/// The `ArtifactBridge` detects `artifacts` and materialises them into the workspace.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolOutput {
    /// Human-readable summary forwarded to the LLM as the tool result.
    pub content: String,
    /// Files produced by this tool invocation. May be empty.
    #[serde(default)]
    pub artifacts: Vec<Artifact>,
    /// Any extra domain metadata. Not sent to the LLM.
    #[serde(default)]
    pub metadata: Value,
}
