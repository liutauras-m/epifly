/// Workspace file indexer — walks a root directory, chunks text content,
/// generates embeddings, and upserts into `content_embeddings`.
///
/// Implements one-shot index and continuous watch modes.  The continuous mode
/// polls for file changes at a configurable interval to avoid external watcher
/// library dependencies.
///
/// This module takes the role of CocoIndex in the architecture: incremental
/// delta-based content ingestion with embedding upserts.
use crate::indexing::EmbeddingService;
use crate::vector_store::PgVectorStore;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::fs;
use tracing::{debug, info, instrument, warn};
use ulid::Ulid;

// ── Configuration ─────────────────────────────────────────────────────────────

const CHUNK_SIZE: usize = 2048; // characters per chunk
const CHUNK_OVERLAP: usize = 128; // character overlap between adjacent chunks
const MAX_CHUNKS: usize = 32; // max chunks per file
const POLL_INTERVAL: Duration = Duration::from_secs(30);

// Supported text file extensions.
const TEXT_EXTS: &[&str] = &[
    "rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "kt", "swift", "md", "txt", "toml", "yaml",
    "yml", "json", "sql", "sh", "html", "css",
];

// ── WorkspaceIndexer ─────────────────────────────────────────────────────────

pub struct WorkspaceIndexer {
    root: PathBuf,
    pool: PgPool,
    embedding_svc: Arc<dyn EmbeddingService>,
    vector_store: Arc<PgVectorStore>,
}

impl WorkspaceIndexer {
    pub fn new(
        root: PathBuf,
        pool: PgPool,
        embedding_svc: Arc<dyn EmbeddingService>,
        vector_store: Arc<PgVectorStore>,
    ) -> Self {
        Self {
            root,
            pool,
            embedding_svc,
            vector_store,
        }
    }

    // ── Public API ────────────────────────────────────────────────────────

    /// Walk the root directory once and index all text files.
    #[instrument(skip(self), fields(root = %self.root.display()))]
    pub async fn index_once(&self) -> anyhow::Result<()> {
        info!(
            "WorkspaceIndexer: starting one-shot index of {}",
            self.root.display()
        );
        let files = self.collect_text_files(&self.root).await?;
        info!(
            file_count = files.len(),
            "WorkspaceIndexer: discovered files"
        );
        for path in files {
            if let Err(e) = self.index_file(&path).await {
                warn!(path = %path.display(), error = %e, "WorkspaceIndexer: failed to index file");
            }
        }
        info!("WorkspaceIndexer: one-shot index complete");
        Ok(())
    }

    /// Continuously poll for file changes and re-index modified files.
    /// Runs until the process exits.  Logs retries on transient errors.
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

    // ── Internals ─────────────────────────────────────────────────────────

    /// Collect all indexable text file paths under `root`.
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
                // Skip hidden directories and common non-source dirs.
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

    /// Index a single file: read content, chunk, embed, upsert.
    async fn index_file(&self, path: &Path) -> anyhow::Result<()> {
        let content = fs::read_to_string(path).await?;
        if content.trim().is_empty() {
            return Ok(());
        }

        // Use file path as a stable synthetic node_id for the indexer.
        let node_id = path_to_node_id(path);
        let chunks = chunk_text(&content);
        debug!(
            path = %path.display(),
            node_id,
            chunks = chunks.len(),
            "WorkspaceIndexer: indexing file"
        );

        // Check content hash — skip if unchanged.
        let content_hash = hex_hash(content.as_bytes());
        if self.is_unchanged(&node_id, &content_hash).await {
            return Ok(());
        }

        let texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
        let embeddings = self.embedding_svc.embed_documents(texts).await?;

        for (i, (chunk, embedding)) in chunks.iter().zip(embeddings.iter()).enumerate() {
            let id = format!("{node_id}_{i}");
            self.vector_store
                .upsert_content_embedding(&id, &node_id, i as i32, &chunk.text, embedding)
                .await?;
        }

        // Remove any stale tail chunks from a previous larger version of this file.
        sqlx::query(
            "DELETE FROM content_embeddings
             WHERE node_id = $1 AND chunk_index >= $2",
        )
        .bind(&node_id)
        .bind(chunks.len() as i32)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Returns `true` when the stored hash for `node_id` matches `hash`.
    async fn is_unchanged(&self, node_id: &str, hash: &str) -> bool {
        let stored_content: Option<String> = sqlx::query_scalar(
            "SELECT string_agg(content, '' ORDER BY chunk_index)
             FROM content_embeddings
             WHERE node_id = $1",
        )
        .bind(node_id)
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten();

        stored_content
            .as_deref()
            .map(|c| hex_hash(c.as_bytes()) == hash)
            .unwrap_or(false)
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
    let canonical = path.to_string_lossy();
    let hash = hex_hash(canonical.as_bytes());
    format!("fsidx_{}", &hash[..16])
}

fn hex_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

// Keep Ulid in scope (used if we switch to ULID-based node IDs in future).
#[allow(dead_code)]
fn new_ulid() -> String {
    Ulid::new().to_string()
}
