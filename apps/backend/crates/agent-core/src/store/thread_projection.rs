//! Durable `thread_projections` index — single source of truth for the
//! `(thread_id → node_id, status, last_seq, content_hash)` mapping.
//!
//! Keyed by `(tenant_id, thread_id)` in a redb table. Values are JSON
//! (same as workspace_nodes) so new optional fields survive rolling restarts.
//!
//! Lookup rules (implemented here):
//! 1. Look up by `(tenant_id, thread_id)`. If present, use stored `node_id`.
//! 2. If absent, derive `node_id` from `blake3(tenant_id ‖ thread_id)` → deterministic Ulid.
//!    Insert the row before returning so subsequent calls take path 1.
//! 3. Never re-derive on every call — that defeats rename preservation.

use anyhow::Context as _;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use redb::{Database, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::task;
use ulid::Ulid;

const TABLE: TableDefinition<(&str, &str), &[u8]> = TableDefinition::new("thread_projections");

// ── Domain types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionStatus {
    #[default]
    Active,
    Paused,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadProjection {
    pub tenant_id: String,
    pub thread_id: String,
    /// Stable node_id — resolved once on first projection and never re-derived.
    pub node_id: Ulid,
    /// Last known virtual path; updated when the node is renamed/moved.
    pub folder_path: String,
    #[serde(default)]
    pub status: ProjectionStatus,
    /// Highest `Message.seq` included in the current revision.
    pub last_seq: u64,
    /// BLAKE3 of the last rendered Markdown body (hex-encoded).
    pub content_hash: String,
    pub message_count: u32,
    pub projected_at: DateTime<Utc>,
    #[serde(default)]
    pub last_error: Option<String>,
}

// ── Trait ─────────────────────────────────────────────────────────────────────

#[async_trait]
pub trait ThreadProjectionStore: Send + Sync {
    /// Resolve (or create) the `ThreadProjection` row for `(tenant_id, thread_id)`.
    ///
    /// If no row exists, inserts a fresh one with a deterministically derived `node_id`
    /// and returns it. Never derives the `node_id` more than once — subsequent calls
    /// always return the stored row.
    async fn resolve_or_create(
        &self,
        tenant_id: &str,
        thread_id: &str,
        initial_folder_path: &str,
    ) -> anyhow::Result<ThreadProjection>;

    async fn get(
        &self,
        tenant_id: &str,
        thread_id: &str,
    ) -> anyhow::Result<Option<ThreadProjection>>;

    async fn upsert(&self, projection: &ThreadProjection) -> anyhow::Result<()>;

    async fn set_status(
        &self,
        tenant_id: &str,
        thread_id: &str,
        status: ProjectionStatus,
    ) -> anyhow::Result<()>;

    async fn update_folder_path(
        &self,
        tenant_id: &str,
        thread_id: &str,
        new_path: &str,
    ) -> anyhow::Result<()>;
}

// ── Redb implementation ───────────────────────────────────────────────────────

pub struct RedbThreadProjectionStore {
    db: Arc<Database>,
}

impl RedbThreadProjectionStore {
    pub fn new(db: Arc<Database>) -> anyhow::Result<Self> {
        // Ensure table exists.
        let wtx = db.begin_write()?;
        wtx.open_table(TABLE)?;
        wtx.commit()?;
        Ok(Self { db })
    }
}

fn ser(v: &ThreadProjection) -> anyhow::Result<Vec<u8>> {
    serde_json::to_vec(v).context("serialize ThreadProjection")
}

fn de(bytes: &[u8]) -> anyhow::Result<ThreadProjection> {
    serde_json::from_slice(bytes).context("deserialize ThreadProjection")
}

/// Derive a deterministic Ulid from `blake3(tenant_id ‖ "\0" ‖ thread_id)`.
/// The first 10 bytes of the hash become the ULID's random component; the
/// timestamp component is set to zero so the id is stable across reboots.
pub fn derive_node_id(tenant_id: &str, thread_id: &str) -> Ulid {
    let mut hasher = blake3::Hasher::new();
    hasher.update(tenant_id.as_bytes());
    hasher.update(b"\0");
    hasher.update(thread_id.as_bytes());
    let hash = hasher.finalize();
    // Take the first 16 bytes of the hash as a u128 seed for Ulid.
    let bytes = hash.as_bytes();
    let hi = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
    let lo = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
    // Ulid is a u128 where the top 48 bits are timestamp and lower 80 are random.
    // We want determinism, so stamp with 0 and fill the random portion with hash bits.
    let raw: u128 = ((hi as u128) << 64) | (lo as u128);
    Ulid::from(raw)
}

#[async_trait]
impl ThreadProjectionStore for RedbThreadProjectionStore {
    async fn resolve_or_create(
        &self,
        tenant_id: &str,
        thread_id: &str,
        initial_folder_path: &str,
    ) -> anyhow::Result<ThreadProjection> {
        let db = Arc::clone(&self.db);
        let tenant = tenant_id.to_owned();
        let thread = thread_id.to_owned();
        let folder = initial_folder_path.to_owned();

        task::spawn_blocking(move || {
            // Try read first (fast path).
            let rtx = db.begin_read()?;
            let tbl = rtx.open_table(TABLE)?;
            if let Some(v) = tbl.get((tenant.as_str(), thread.as_str()))? {
                return de(v.value());
            }
            drop(tbl);
            drop(rtx);

            // Insert new row.
            let node_id = derive_node_id(&tenant, &thread);
            let projection = ThreadProjection {
                tenant_id: tenant.clone(),
                thread_id: thread.clone(),
                node_id,
                folder_path: folder,
                status: ProjectionStatus::Active,
                last_seq: 0,
                content_hash: String::new(),
                message_count: 0,
                projected_at: Utc::now(),
                last_error: None,
            };
            let bytes = ser(&projection)?;
            let wtx = db.begin_write()?;
            {
                let mut tbl = wtx.open_table(TABLE)?;
                tbl.insert((tenant.as_str(), thread.as_str()), bytes.as_slice())?;
            }
            wtx.commit()?;
            Ok(projection)
        })
        .await
        .context("thread_projection resolve_or_create spawn")?
    }

    async fn get(
        &self,
        tenant_id: &str,
        thread_id: &str,
    ) -> anyhow::Result<Option<ThreadProjection>> {
        let db = Arc::clone(&self.db);
        let tenant = tenant_id.to_owned();
        let thread = thread_id.to_owned();
        task::spawn_blocking(move || {
            let rtx = db.begin_read()?;
            let tbl = rtx.open_table(TABLE)?;
            match tbl.get((tenant.as_str(), thread.as_str()))? {
                Some(v) => Ok(Some(de(v.value())?)),
                None => Ok(None),
            }
        })
        .await
        .context("thread_projection get spawn")?
    }

    async fn upsert(&self, projection: &ThreadProjection) -> anyhow::Result<()> {
        let db = Arc::clone(&self.db);
        let bytes = ser(projection)?;
        let tenant = projection.tenant_id.clone();
        let thread = projection.thread_id.clone();
        task::spawn_blocking(move || {
            let wtx = db.begin_write()?;
            {
                let mut tbl = wtx.open_table(TABLE)?;
                tbl.insert((tenant.as_str(), thread.as_str()), bytes.as_slice())?;
            }
            wtx.commit().context("thread_projection upsert commit")
        })
        .await
        .context("thread_projection upsert spawn")?
    }

    async fn set_status(
        &self,
        tenant_id: &str,
        thread_id: &str,
        status: ProjectionStatus,
    ) -> anyhow::Result<()> {
        let db = Arc::clone(&self.db);
        let tenant = tenant_id.to_owned();
        let thread = thread_id.to_owned();
        task::spawn_blocking(move || {
            let wtx = db.begin_write()?;
            {
                let mut tbl = wtx.open_table(TABLE)?;
                let mut proj = match tbl.get((tenant.as_str(), thread.as_str()))? {
                    Some(v) => de(v.value())?,
                    None => {
                        return Err(anyhow::anyhow!(
                            "thread_projection not found: {tenant}/{thread}"
                        ));
                    }
                };
                proj.status = status;
                let bytes = ser(&proj)?;
                tbl.insert((tenant.as_str(), thread.as_str()), bytes.as_slice())?;
            }
            wtx.commit().context("thread_projection set_status commit")
        })
        .await
        .context("thread_projection set_status spawn")?
    }

    async fn update_folder_path(
        &self,
        tenant_id: &str,
        thread_id: &str,
        new_path: &str,
    ) -> anyhow::Result<()> {
        let db = Arc::clone(&self.db);
        let tenant = tenant_id.to_owned();
        let thread = thread_id.to_owned();
        let path = new_path.to_owned();
        task::spawn_blocking(move || {
            let wtx = db.begin_write()?;
            {
                let mut tbl = wtx.open_table(TABLE)?;
                let mut proj = match tbl.get((tenant.as_str(), thread.as_str()))? {
                    Some(v) => de(v.value())?,
                    None => {
                        return Err(anyhow::anyhow!(
                            "thread_projection not found: {tenant}/{thread}"
                        ));
                    }
                };
                proj.folder_path = path;
                let bytes = ser(&proj)?;
                tbl.insert((tenant.as_str(), thread.as_str()), bytes.as_slice())?;
            }
            wtx.commit()
                .context("thread_projection update_folder_path commit")
        })
        .await
        .context("thread_projection update_folder_path spawn")?
    }
}

// ── In-memory implementation (tests) ──────────────────────────────────────────

pub struct InMemoryThreadProjectionStore {
    rows: parking_lot::Mutex<std::collections::HashMap<(String, String), ThreadProjection>>,
}

impl InMemoryThreadProjectionStore {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            rows: parking_lot::Mutex::new(std::collections::HashMap::new()),
        })
    }
}

impl Default for InMemoryThreadProjectionStore {
    fn default() -> Self {
        Self {
            rows: parking_lot::Mutex::new(std::collections::HashMap::new()),
        }
    }
}

#[async_trait]
impl ThreadProjectionStore for InMemoryThreadProjectionStore {
    async fn resolve_or_create(
        &self,
        tenant_id: &str,
        thread_id: &str,
        initial_folder_path: &str,
    ) -> anyhow::Result<ThreadProjection> {
        let key = (tenant_id.to_owned(), thread_id.to_owned());
        let mut rows = self.rows.lock();
        if let Some(p) = rows.get(&key) {
            return Ok(p.clone());
        }
        let projection = ThreadProjection {
            tenant_id: tenant_id.to_owned(),
            thread_id: thread_id.to_owned(),
            node_id: derive_node_id(tenant_id, thread_id),
            folder_path: initial_folder_path.to_owned(),
            status: ProjectionStatus::Active,
            last_seq: 0,
            content_hash: String::new(),
            message_count: 0,
            projected_at: Utc::now(),
            last_error: None,
        };
        rows.insert(key, projection.clone());
        Ok(projection)
    }

    async fn get(
        &self,
        tenant_id: &str,
        thread_id: &str,
    ) -> anyhow::Result<Option<ThreadProjection>> {
        Ok(self
            .rows
            .lock()
            .get(&(tenant_id.to_owned(), thread_id.to_owned()))
            .cloned())
    }

    async fn upsert(&self, projection: &ThreadProjection) -> anyhow::Result<()> {
        self.rows.lock().insert(
            (projection.tenant_id.clone(), projection.thread_id.clone()),
            projection.clone(),
        );
        Ok(())
    }

    async fn set_status(
        &self,
        tenant_id: &str,
        thread_id: &str,
        status: ProjectionStatus,
    ) -> anyhow::Result<()> {
        let key = (tenant_id.to_owned(), thread_id.to_owned());
        let mut rows = self.rows.lock();
        let proj = rows
            .get_mut(&key)
            .ok_or_else(|| anyhow::anyhow!("thread_projection not found"))?;
        proj.status = status;
        Ok(())
    }

    async fn update_folder_path(
        &self,
        tenant_id: &str,
        thread_id: &str,
        new_path: &str,
    ) -> anyhow::Result<()> {
        let key = (tenant_id.to_owned(), thread_id.to_owned());
        let mut rows = self.rows.lock();
        let proj = rows
            .get_mut(&key)
            .ok_or_else(|| anyhow::anyhow!("thread_projection not found"))?;
        proj.folder_path = new_path.to_owned();
        Ok(())
    }
}

// ── Factory ───────────────────────────────────────────────────────────────────

/// Selects which storage backend to use when constructing a [`ThreadProjectionStore`].
///
/// Callers (e.g. `AppState`) depend on this enum instead of constructing concrete
/// store types directly — the factory is the single place to add new backends.
pub enum ProjectionStoreBackend {
    /// Persistent storage backed by a shared redb database.
    Redb(Arc<redb::Database>),
    /// Ephemeral in-memory storage — suitable for tests and in-memory deployments.
    InMemory,
}

/// Construct a [`ThreadProjectionStore`] for the given backend.
///
/// The returned `Arc<dyn ThreadProjectionStore>` is type-erased so call sites
/// never need to import the concrete implementation types.
pub fn build_thread_projection_store(
    backend: ProjectionStoreBackend,
) -> anyhow::Result<Arc<dyn ThreadProjectionStore>> {
    match backend {
        ProjectionStoreBackend::Redb(db) => Ok(Arc::new(RedbThreadProjectionStore::new(db)?)),
        ProjectionStoreBackend::InMemory => Ok(InMemoryThreadProjectionStore::new()),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── derive_node_id unit tests ─────────────────────────────────────────────

    #[test]
    fn derive_node_id_is_stable() {
        let a = derive_node_id("acme", "thread-1");
        let b = derive_node_id("acme", "thread-1");
        assert_eq!(a, b);
    }

    #[test]
    fn derive_node_id_differs_by_tenant() {
        let a = derive_node_id("acme", "thread-1");
        let b = derive_node_id("other", "thread-1");
        assert_ne!(a, b);
    }

    #[test]
    fn derive_node_id_differs_by_thread() {
        let a = derive_node_id("acme", "thread-1");
        let b = derive_node_id("acme", "thread-2");
        assert_ne!(a, b);
    }

    // ── Contract test suite ───────────────────────────────────────────────────
    //
    // These six assertions must hold for every ThreadProjectionStore backend.
    // Call `projection_store_contract` from a backend-specific test to verify.

    async fn projection_store_contract(store: Arc<dyn ThreadProjectionStore>) {
        // 1. resolve_or_create derives a deterministic node_id on first call.
        let p1 = store
            .resolve_or_create("acme", "t1", "Conversations")
            .await
            .unwrap();
        assert_eq!(
            p1.node_id,
            derive_node_id("acme", "t1"),
            "node_id must equal derive_node_id(tenant, thread)"
        );

        // 2. Second resolve_or_create preserves node_id and does NOT overwrite folder_path.
        let p2 = store
            .resolve_or_create("acme", "t1", "Somewhere Else")
            .await
            .unwrap();
        assert_eq!(
            p1.node_id, p2.node_id,
            "node_id must be stable across calls"
        );
        assert_eq!(
            p2.folder_path, "Conversations",
            "original folder_path must not be overwritten on second resolve"
        );

        // 3. set_status writes through and get reflects the change.
        store
            .set_status("acme", "t1", ProjectionStatus::Paused)
            .await
            .unwrap();
        let after_pause = store.get("acme", "t1").await.unwrap().unwrap();
        assert_eq!(
            after_pause.status,
            ProjectionStatus::Paused,
            "set_status must persist"
        );

        // 4. update_folder_path writes through and get reflects the change.
        store
            .update_folder_path("acme", "t1", "New Path")
            .await
            .unwrap();
        let after_rename = store.get("acme", "t1").await.unwrap().unwrap();
        assert_eq!(
            after_rename.folder_path, "New Path",
            "update_folder_path must persist"
        );

        // 5. get returns None for a projection that does not exist.
        let missing = store.get("acme", "does-not-exist").await.unwrap();
        assert!(
            missing.is_none(),
            "get must return None for an unknown thread"
        );

        // 6. Update operations return Err when the projection does not exist.
        let set_err = store
            .set_status("acme", "missing-thread", ProjectionStatus::Error)
            .await;
        assert!(
            set_err.is_err(),
            "set_status on a missing projection must return Err"
        );
        let path_err = store
            .update_folder_path("acme", "missing-thread", "/new")
            .await;
        assert!(
            path_err.is_err(),
            "update_folder_path on a missing projection must return Err"
        );
    }

    #[tokio::test]
    async fn contract_inmemory() {
        let store = build_thread_projection_store(ProjectionStoreBackend::InMemory).unwrap();
        projection_store_contract(store).await;
    }

    #[tokio::test]
    async fn contract_redb() {
        // Use redb's in-memory backend — no filesystem, no dev-dependency on tempfile.
        let db = Arc::new(
            redb::Database::builder()
                .create_with_backend(redb::backends::InMemoryBackend::new())
                .expect("in-memory redb for contract test"),
        );
        let store = build_thread_projection_store(ProjectionStoreBackend::Redb(db)).unwrap();
        projection_store_contract(store).await;
    }
}
