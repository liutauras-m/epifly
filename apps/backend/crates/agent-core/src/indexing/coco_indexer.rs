/// Workspace file indexer — walks a root directory, chunks text content,
/// generates embeddings, and upserts into the Qdrant `content_embeddings` collection.
///
/// Implements one-shot index and continuous watch modes.
use crate::indexing::EmbeddingService;
use crate::store::qdrant_vector::QdrantVectorStore;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::fs;
use tracing::{debug, info, instrument, warn};
use ulid::Ulid;

// ── Configuration ─────────────────────────────────────────────────────────────

const CHUNK_SIZE: usize = 2048;
const CHUNK_OVERLAP: usize = 128;
const MAX_CHUNKS: usize = 32;
const POLL_INTERVAL: Duration = Duration::from_secs(30);

const TEXT_EXTS: &[&str] = &[
    "rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "kt", "swift", "md", "txt", "toml", "yaml",
    "yml", "json", "sql", "sh", "html", "css",
];

// ── WorkspaceIndexer ─────────────────────────────────────────────────────────

pub struct WorkspaceIndexer {
    root: PathBuf,
    embedding_svc: Arc<dyn EmbeddingService>,
    vector_store: Arc<QdrantVectorStore>,
}

impl WorkspaceIndexer {
    pub fn new(
        root: PathBuf,
        embedding_svc: Arc<dyn EmbeddingService>,
        vector_store: Arc<QdrantVectorStore>,
    ) -> Self {
        Self {
            root,
            embedding_svc,
            vector_store,
        }
    }

    /// Walk the root directory once and index all text files.
    #[instrument(skip(self), fields(root = %self.root.display()))]
    pub async fn index_once(&self) -> anyhow::Result<()> {
        info!(
            "WorkspaceIndexer: starting one-shot index of {}",
            self.root.display()
        );
        let files = self.collect_text_files(&self.root).await?;
        info!(file_count = files.len(), "WorkspaceIndexer: discovered files");
        for path in files {
            if let Err(e) = self.index_file(&path).await {
                warn!(path = %path.display(), error = %e, "WorkspaceIndexer: failed to index file");
            }
        }
        info!("WorkspaceIndexer: one-shot index complete");
        Ok(())
    }

    /// Continuously poll for file changes and re-index modified files.
    #[instrument(skip(self), fields(root = %self.root.display()))]
    pub async fn watch_and_index(self: Arc<Self>) {
        info!(
            root = %self.root.display(),
            interval_s = POLL_INTERVAL.as_secs(),
            "WorkspaceIndexer: starting continuous watcher"
        );

        let mut consecutive_errors: u32 = 0;
        loop {
            tokio::time::sleep(POLL_INTERVAL).await;

            match self.index_once().await {
                Ok(()) => {
                    consecutive_errors = 0;
                }
                Err(e) => {
                    consecutive_errors += 1;
                    let backoff = std::cmp::min(consecutive_errors * 10, 300);
                    warn!(
                        error = %e,
                        consecutive_errors,
                        retry_in_s = backoff,
                        "WorkspaceIndexer: transient error — will retry"
                    );
                    tokio::time::sleep(Duration::from_secs(u64::from(backoff))).await;
                }
            }
        }
    }

    async fn collect_text_files(&self, dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        self.walk_dir(dir, &mut files).await?;
        Ok(files)
    }

    #[async_recursion::async_recursion]
    async fn walk_dir(&self, dir: &Path, acc: &mut Vec<PathBuf>) -> anyhow::Result<()> {
        let mut entries = fs::read_dir(dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.starts_with('.') || matches!(name, "target" | "node_modules" | "dist") {
                    continue;
                }
                self.walk_dir(&path, acc).await?;
            } else if self.is_text_file(&path) {
                acc.push(path);
            }
        }
        Ok(())
    }

    fn is_text_file(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|ext| TEXT_EXTS.contains(&ext))
            .unwrap_or(false)
    }

    async fn index_file(&self, path: &Path) -> anyhow::Result<()> {
        let content = fs::read_to_string(path).await?;
        if content.trim().is_empty() {
            return Ok(());
        }

        let node_id = path_to_node_id(path);
        let chunks = chunk_text(&content);
        debug!(
            path = %path.display(),
            node_id,
            chunks = chunks.len(),
            "WorkspaceIndexer: indexing file"
        );

        let texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
        let embeddings = self.embedding_svc.embed_documents(texts).await?;

        for (i, (chunk, embedding)) in chunks.iter().zip(embeddings.iter()).enumerate() {
            let chunk_id = format!("{node_id}_{i}");
            self.vector_store
                .upsert_content_embedding(&chunk_id, &node_id, i as i32, &chunk.text, &embedding)
                .await?;
        }

        Ok(())
    }
}

// ── Text chunking ─────────────────────────────────────────────────────────────

struct Chunk {
    text: String,
}

fn chunk_text(content: &str) -> Vec<Chunk> {
    let chars: Vec<char> = content.chars().collect();
    let total = chars.len();
    if total == 0 {
        return vec![];
    }

    let mut chunks = Vec::new();
    let mut start = 0usize;

    while start < total && chunks.len() < MAX_CHUNKS {
        let end = (start + CHUNK_SIZE).min(total);
        let text: String = chars[start..end].iter().collect();
        chunks.push(Chunk { text });
        if end == total {
            break;
        }
        start = end.saturating_sub(CHUNK_OVERLAP);
    }
    chunks
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn path_to_node_id(path: &Path) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    path.to_string_lossy().as_ref().hash(&mut h);
    format!("fsidx_{:016x}", h.finish())
}

#[allow(dead_code)]
fn new_ulid() -> String {
    Ulid::new().to_string()
}
