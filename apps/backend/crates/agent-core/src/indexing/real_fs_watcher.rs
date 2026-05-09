/// Lightweight polling file-system watcher.
///
/// Wraps `WorkspaceIndexer::watch_and_index` and provides a clean start/stop
/// interface so the gateway can manage the watcher's lifecycle.
use crate::indexing::WorkspaceIndexer;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::info;

pub struct RealFsWatcher {
    handle: JoinHandle<()>,
}

impl RealFsWatcher {
    /// Spawn the watcher task and return immediately.
    pub fn spawn(indexer: Arc<WorkspaceIndexer>) -> Self {
        let handle = tokio::spawn(async move {
            indexer.watch_and_index().await;
        });
        info!("RealFsWatcher: watcher task spawned");
        Self { handle }
    }

    /// Abort the watcher task.
    pub fn stop(self) {
        self.handle.abort();
        info!("RealFsWatcher: watcher task stopped");
    }
}
