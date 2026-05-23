//! Post-invoke artifact materialisation bridge.
//!
//! Called after every `CapabilityProvider::invoke()` that returns a `ToolOutput`
//! with non-empty `artifacts`. Uploads binaries to the object store and inserts
//! workspace_nodes rows via `RedbMetadataStore`.

use base64::Engine as _;
use common::artifact::{Artifact, ToolOutput};
use common::memory::store::WorkspaceContentStore;
use object_store::{ObjectStore, path::Path as OsPath};
use std::sync::Arc;
use tracing::{info, instrument, warn};
use ulid::Ulid;

const INDEXABLE_MIME_PREFIXES: &[&str] = &["text/", "application/pdf", "application/json"];

pub struct ArtifactBridge {
    object_store: Arc<dyn ObjectStore>,
    content_store: Arc<dyn WorkspaceContentStore>,
    /// Optional base URL for publicly reachable artifacts (e.g. `http://localhost:9000/conusai`).
    /// Read from `RUSTFS_PUBLIC_BASE_URL` at startup. Required for static hosting.
    pub_base_url: Option<String>,
}

impl ArtifactBridge {
    pub fn new(
        object_store: Arc<dyn ObjectStore>,
        content_store: Arc<dyn WorkspaceContentStore>,
    ) -> Arc<Self> {
        let pub_base_url = std::env::var("RUSTFS_PUBLIC_BASE_URL").ok();
        Arc::new(Self { object_store, content_store, pub_base_url })
    }

    /// Materialise all artifacts in `output`. Failures are logged, not propagated.
    ///
    /// Returns `(public_url, changed_paths)` where:
    /// - `public_url` is `Some(url)` when `output.metadata.hosting_type == "static"` and
    ///   `RUSTFS_PUBLIC_BASE_URL` is configured.
    /// - `changed_paths` is the deduplicated list of virtual paths written to the workspace
    ///   content store (text/JSON artifacts only). Used by the streaming path to emit a
    ///   `resource_invalidated` SSE delta so clients can revalidate (PR 3.A).
    ///
    /// `output.metadata["artifact_path_prefix"]` overrides the virtual path prefix:
    /// - absent → `/outputs/{tool_name}/{artifact.name}` (default)
    /// - `""` → `/{artifact.name}` (raw project paths, used by code-project)
    #[instrument(skip(self, output), fields(tool = tool_name, artifact_count = output.artifacts.len()))]
    pub async fn process_if_artifacts(
        &self,
        tenant_id: &str,
        user_id: Option<&str>,
        tool_name: &str,
        parent_node_id: Option<&str>,
        output: &ToolOutput,
    ) -> anyhow::Result<(Option<String>, Vec<String>)> {
        if output.artifacts.is_empty() {
            return Ok((None, vec![]));
        }

        let hosting = output.metadata["hosting_type"].as_str() == Some("static");
        let path_prefix = output.metadata["artifact_path_prefix"].as_str();
        let mut changed_paths: Vec<String> = Vec::new();

        for artifact in &output.artifacts {
            match self
                .materialise(tenant_id, user_id, tool_name, parent_node_id, artifact, path_prefix, hosting)
                .await
            {
                Ok(Some(vpath)) => changed_paths.push(vpath),
                Ok(None) => {}
                Err(e) => {
                    warn!(error = %e, artifact = %artifact.name, "artifact materialisation failed — skipping");
                }
            }
        }

        // Derive public URL for static hosting when a base URL is configured.
        let public_url = if hosting {
            if let Some(ref base) = self.pub_base_url {
                let root_path = output.metadata["root_path"].as_str().unwrap_or("");
                let index = output.metadata["index_file"].as_str().unwrap_or("index.html");
                Some(format!("{base}/{tenant_id}/static/{root_path}/{index}"))
            } else {
                warn!("hosting_type=static but RUSTFS_PUBLIC_BASE_URL is not set — no public_url returned");
                None
            }
        } else {
            None
        };

        Ok((public_url, changed_paths))
    }

    /// Returns `Ok(Some(virtual_path))` when a text artifact was written to the
    /// content store (so the caller can collect changed paths for `resource_invalidated`).
    /// Returns `Ok(None)` for binary artifacts or when no content-store write occurred.
    async fn materialise(
        &self,
        tenant_id: &str,
        _user_id: Option<&str>,
        tool_name: &str,
        _parent_node_id: Option<&str>,
        artifact: &Artifact,
        path_prefix: Option<&str>,
        hosting: bool,
    ) -> anyhow::Result<Option<String>> {
        // Stable object key for hosting artifacts (no ULID — overwrites on redeploy).
        let object_key = if hosting {
            format!("{tenant_id}/static/{}", artifact.name)
        } else {
            let node_id = Ulid::new().to_string();
            format!("{tenant_id}/{tool_name}/{node_id}/{}", artifact.name)
        };

        // Decode artifact data: try base64 first, fall back to treating as plain UTF-8.
        // Chain capabilities (e.g. code-project) return plain text; binary providers use base64.
        let decode_bytes = |s: &str| -> Vec<u8> {
            base64::engine::general_purpose::STANDARD
                .decode(s)
                .unwrap_or_else(|_| s.as_bytes().to_vec())
        };

        // Upload to object store if data is present.
        if let Some(ref raw) = artifact.data {
            let bytes = decode_bytes(raw);
            self.object_store
                .put(&OsPath::from(object_key.as_str()), bytes.into())
                .await?;
        }

        // Write text artifacts to workspace content store; return the virtual path written.
        let written_path = if is_indexable(&artifact.mime_type) {
            if let Some(ref raw) = artifact.data {
                let bytes = decode_bytes(raw);
                if let Ok(text) = std::str::from_utf8(&bytes) {
                    // Allow caller to override virtual path prefix via output metadata.
                    let virtual_path = match path_prefix {
                        Some(pfx) => format!("{pfx}/{}", artifact.name),
                        None => format!("/outputs/{tool_name}/{}", artifact.name),
                    };
                    let _ = self
                        .content_store
                        .write(tenant_id, &virtual_path, text)
                        .await;
                    Some(virtual_path)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        info!(artifact = %artifact.name, object_key = %object_key, "artifact materialised");
        Ok(written_path)
    }
}

fn is_indexable(mime: &str) -> bool {
    INDEXABLE_MIME_PREFIXES.iter().any(|p| mime.starts_with(p))
}
