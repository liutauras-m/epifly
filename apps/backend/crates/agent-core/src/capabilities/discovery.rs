use super::registry::CapabilityRegistry;
use crate::realtime::{RealtimeService, WorkspaceChangeEvent};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{info, warn};

pub struct CapabilityDiscovery {
    dirs: Vec<PathBuf>,
}

impl CapabilityDiscovery {
    pub fn new(dirs: Vec<PathBuf>) -> Self {
        Self { dirs }
    }

    pub fn from_env() -> Self {
        let dir = std::env::var("CONUSAI_CAPABILITIES_DIR")
            .unwrap_or_else(|_| "./capabilities".to_string());
        Self::new(vec![PathBuf::from(dir)])
    }

    pub fn discover(&self) -> common::error::Result<CapabilityRegistry> {
        let mut registry = CapabilityRegistry::new();
        self.discover_into(&mut registry)?;
        Ok(registry)
    }

    /// Discover capabilities into an existing registry (preserves pre-registered factories
    /// and providers).  Use with `CapabilityRegistry::with_default_factories()` so YAML-loaded
    /// capabilities receive the correct provider factories.
    pub fn discover_into(&self, registry: &mut CapabilityRegistry) -> common::error::Result<()> {
        let mut total = 0;
        for dir in &self.dirs {
            let count = registry.load_from_dir(dir)?;
            info!(dir = ?dir, count, "discovered tools");
            total += count;
        }
        info!(total, "tool discovery complete");
        Ok(())
    }
}

/// Watches capability directories for `capability.toml` changes and hot-reloads the registry.
///
/// Holds the underlying `notify` watcher alive for the process lifetime.
/// Drop to stop watching.
pub struct ManifestWatcher {
    _watcher: RecommendedWatcher,
}

impl ManifestWatcher {
    /// Start watching all dirs returned by `CapabilityDiscovery::from_env()`.
    ///
    /// Changes to any `capability.toml` file trigger a debounced reload of the affected
    /// capability directory into `registry`. Uses a 250 ms debounce to coalesce rapid saves.
    /// When `realtime` is provided, a `capability.reloaded` event is broadcast on the
    /// `__system__` channel after each successful reload.
    pub fn start(
        registry: Arc<Mutex<CapabilityRegistry>>,
        realtime: Option<Arc<RealtimeService>>,
    ) -> anyhow::Result<Self> {
        let discovery = CapabilityDiscovery::from_env();
        Self::start_for_dirs(registry, discovery.dirs, realtime)
    }

    pub fn start_for_dirs(
        registry: Arc<Mutex<CapabilityRegistry>>,
        dirs: Vec<PathBuf>,
        realtime: Option<Arc<RealtimeService>>,
    ) -> anyhow::Result<Self> {
        // Channel carries the parent directory of a changed capability.toml.
        let (tx, rx) = std::sync::mpsc::channel::<PathBuf>();

        let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            match res {
                Ok(event) => {
                    if !matches!(
                        event.kind,
                        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
                    ) {
                        return;
                    }
                    for path in &event.paths {
                        if path.file_name() == Some(std::ffi::OsStr::new("capability.toml")) {
                            if let Some(cap_dir) = path.parent().map(|p| p.to_path_buf()) {
                                let _ = tx.send(cap_dir);
                            }
                        }
                    }
                }
                Err(e) => warn!(error = %e, "manifest watcher error"),
            }
        })?;

        for dir in &dirs {
            if dir.exists() {
                watcher.watch(dir, RecursiveMode::Recursive)?;
                info!(dir = ?dir, "watching capability directory for hot-reload");
            }
        }

        // Debounce: collect events for 250 ms then process.
        std::thread::spawn(move || {
            while let Ok(cap_dir) = rx.recv() {
                // Drain any additional events queued in the debounce window.
                std::thread::sleep(Duration::from_millis(250));
                while rx.try_recv().is_ok() {}

                info!(dir = ?cap_dir, "hot-reloading capability");
                let cap_name = cap_dir
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "unknown".into());

                let reloaded = match registry.lock() {
                    Ok(mut reg) => match reg.reload_capability(&cap_dir) {
                        Ok(()) => true,
                        Err(e) => {
                            warn!(dir = ?cap_dir, error = %e, "hot-reload failed");
                            false
                        }
                    },
                    Err(e) => {
                        warn!(error = %e, "registry lock poisoned during hot-reload");
                        false
                    }
                };

                // Emit capability.reloaded on the realtime bus (system channel).
                if reloaded {
                    if let Some(rt) = realtime.as_ref() {
                        if let Ok(handle) = tokio::runtime::Handle::try_current() {
                            let rt = Arc::clone(rt);
                            let name = cap_name.clone();
                            handle.spawn(async move {
                                rt.publish_workspace_change(WorkspaceChangeEvent {
                                    op: "capability.reloaded".into(),
                                    tenant_id: "__system__".into(),
                                    node_id: name,
                                    kind: "capability".into(),
                                })
                                .await;
                            });
                        }
                    }
                }
            }
        });

        Ok(Self { _watcher: watcher })
    }
}
