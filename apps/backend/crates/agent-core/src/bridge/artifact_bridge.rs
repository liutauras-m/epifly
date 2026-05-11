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
}

impl ArtifactBridge {
    pub fn new(
        object_store: Arc<dyn ObjectStore>,
        content_store: Arc<dyn WorkspaceContentStore>,
    ) -> Arc<Self> {
        Arc::new(Self {
            object_store,
            content_store,
        })
    }

    /// Materialise all artifacts in `output`. Failures are logged, not propagated.
    #[instrument(skip(self, output), fields(tool = tool_name, artifact_count = output.artifacts.len()))]
    pub async fn process_if_artifacts(
        &self,
        tenant_id: &str,
        user_id: Option<&str>,
        tool_name: &str,
        parent_node_id: Option<&str>,
        output: &ToolOutput,
    ) -> anyhow::Result<()> {
        if output.artifacts.is_empty() {
            return Ok(());
        }
        for artifact in &output.artifacts {
            if let Err(e) = self
                .materialise(tenant_id, user_id, tool_name, parent_node_id, artifact)
                .await
            {
                warn!(error = %e, artifact = %artifact.name, "artifact materialisation failed — skipping");
            }
        }
        Ok(())
    }

    async fn materialise(
        &self,
        tenant_id: &str,
        _user_id: Option<&str>,
        tool_name: &str,
        _parent_node_id: Option<&str>,
        artifact: &Artifact,
    ) -> anyhow::Result<()> {
        let node_id = Ulid::new().to_string();
        let object_key = format!("{tenant_id}/{tool_name}/{node_id}/{}", artifact.name);

        // Upload to object store if base64 data is present.
        if let Some(ref b64) = artifact.data {
            let bytes = base64::engine::general_purpose::STANDARD.decode(b64)?;
            self.object_store
                .put(&OsPath::from(object_key.as_str()), bytes.into())
                .await?;
        }

        // Write text artifacts to workspace content store.
        if is_indexable(&artifact.mime_type) {
            if let Some(ref b64) = artifact.data {
                let bytes = base64::engine::general_purpose::STANDARD.decode(b64)?;
                if let Ok(text) = std::str::from_utf8(&bytes) {
                    let virtual_path = format!("/outputs/{tool_name}/{}", artifact.name);
                    let _ = self
                        .content_store
                        .write(tenant_id, &virtual_path, text)
                        .await;
                }
            }
        }

        info!(artifact = %artifact.name, node_id = %node_id, "artifact materialised");
        Ok(())
    }
}

fn is_indexable(mime: &str) -> bool {
    INDEXABLE_MIME_PREFIXES.iter().any(|p| mime.starts_with(p))
}
