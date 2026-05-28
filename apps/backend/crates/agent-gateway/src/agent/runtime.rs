//! `ThreadRuntime` — per-thread in-memory transient state.
//!
//! Holds only derived/transient data. Never the only copy of a message.
//! `AgentTurnRunner` still persists user + assistant messages synchronously
//! via `agent::persistence` before/after the model call.
//!
//! ## Rules
//! - GC: background task evicts runtimes idle > 15 minutes.
//! - Stop-button from any device cancels the active stream (cancellation token
//!   is owned here, not by the request).
//! - Registry must NOT be used as a write-cache for messages.

use dashmap::DashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use tokio_util::sync::CancellationToken;

// ── Domain types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamState {
    Idle,
    Running { run_id: String },
    Stopped,
}

pub struct ThreadRuntime {
    pub tenant_id: String,
    pub thread_id: String,
    /// The run currently in flight, if any.
    pub active_run_id: parking_lot::RwLock<Option<String>>,
    pub stream_state: parking_lot::RwLock<StreamState>,
    /// Cancellation token for the active stream. Replaced on each new run.
    pub cancellation: parking_lot::RwLock<CancellationToken>,
    /// Unix timestamp (seconds) of last activity.
    pub last_activity: AtomicI64,
}

impl ThreadRuntime {
    pub fn new(tenant_id: impl Into<String>, thread_id: impl Into<String>) -> Arc<Self> {
        Arc::new(Self {
            tenant_id: tenant_id.into(),
            thread_id: thread_id.into(),
            active_run_id: parking_lot::RwLock::new(None),
            stream_state: parking_lot::RwLock::new(StreamState::Idle),
            cancellation: parking_lot::RwLock::new(CancellationToken::new()),
            last_activity: AtomicI64::new(now_secs()),
        })
    }

    /// Touch the last-activity timestamp.
    pub fn touch(&self) {
        self.last_activity.store(now_secs(), Ordering::Relaxed);
    }

    /// Seconds since last activity.
    pub fn idle_secs(&self) -> i64 {
        now_secs() - self.last_activity.load(Ordering::Relaxed)
    }

    /// Start a new run: replace cancellation token, set state to Running.
    pub fn start_run(&self, run_id: String) -> CancellationToken {
        let token = CancellationToken::new();
        *self.cancellation.write() = token.clone();
        *self.active_run_id.write() = Some(run_id.clone());
        *self.stream_state.write() = StreamState::Running { run_id };
        self.touch();
        token
    }

    /// Mark the run as idle (done or stopped).
    pub fn finish_run(&self) {
        *self.active_run_id.write() = None;
        *self.stream_state.write() = StreamState::Idle;
        self.touch();
    }

    /// Cancel the active run (stop-button).
    pub fn cancel(&self) {
        self.cancellation.read().cancel();
        *self.stream_state.write() = StreamState::Stopped;
        self.touch();
    }
}

// ── Registry ──────────────────────────────────────────────────────────────────

/// GC interval and idle timeout constants.
const GC_INTERVAL_SECS: u64 = 60;
const IDLE_TTL_SECS: i64 = 900; // 15 minutes

pub struct ThreadRuntimeRegistry {
    runtimes: DashMap<(String, String), Arc<ThreadRuntime>>,
}

impl ThreadRuntimeRegistry {
    pub fn new() -> Arc<Self> {
        let registry = Arc::new(Self {
            runtimes: DashMap::new(),
        });
        // Spawn GC background task.
        let weak = Arc::downgrade(&registry);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(GC_INTERVAL_SECS)).await;
                let Some(r) = weak.upgrade() else { break };
                r.gc();
            }
        });
        registry
    }

    /// Get an existing runtime or create one on demand.
    pub fn get_or_create(&self, tenant_id: &str, thread_id: &str) -> Arc<ThreadRuntime> {
        let key = (tenant_id.to_owned(), thread_id.to_owned());
        self.runtimes
            .entry(key)
            .or_insert_with(|| ThreadRuntime::new(tenant_id, thread_id))
            .clone()
    }

    /// Get an existing runtime without creating one.
    pub fn get(&self, tenant_id: &str, thread_id: &str) -> Option<Arc<ThreadRuntime>> {
        self.runtimes
            .get(&(tenant_id.to_owned(), thread_id.to_owned()))
            .map(|r| r.clone())
    }

    /// Cancel the active stream for a thread (stop-button from any device).
    pub fn cancel(&self, tenant_id: &str, thread_id: &str) -> bool {
        if let Some(rt) = self.get(tenant_id, thread_id) {
            rt.cancel();
            true
        } else {
            false
        }
    }

    /// Evict runtimes that have been idle longer than `IDLE_TTL_SECS`.
    fn gc(&self) {
        self.runtimes.retain(|_, rt| rt.idle_secs() < IDLE_TTL_SECS);
    }

    /// Number of currently registered runtimes (for metrics/health).
    pub fn len(&self) -> usize {
        self.runtimes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.runtimes.is_empty()
    }
}

impl Default for ThreadRuntimeRegistry {
    fn default() -> Self {
        Self {
            runtimes: DashMap::new(),
        }
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_and_cancel_run() {
        let rt = ThreadRuntime::new("acme", "t1");
        let token = rt.start_run("run-1".into());
        assert!(!token.is_cancelled());
        rt.cancel();
        assert!(token.is_cancelled());
        assert_eq!(*rt.stream_state.read(), StreamState::Stopped);
    }

    #[test]
    fn finish_run_clears_active_run_id() {
        let rt = ThreadRuntime::new("acme", "t1");
        rt.start_run("run-1".into());
        assert!(rt.active_run_id.read().is_some());
        rt.finish_run();
        assert!(rt.active_run_id.read().is_none());
        assert_eq!(*rt.stream_state.read(), StreamState::Idle);
    }

    #[tokio::test]
    async fn registry_get_or_create_is_idempotent() {
        let reg = ThreadRuntimeRegistry::new();
        let a = reg.get_or_create("acme", "t1");
        let b = reg.get_or_create("acme", "t1");
        assert!(Arc::ptr_eq(&a, &b), "same runtime returned on second call");
    }

    #[tokio::test]
    async fn registry_cancel_returns_false_for_missing() {
        let reg = ThreadRuntimeRegistry::new();
        assert!(!reg.cancel("acme", "missing"));
    }

    #[tokio::test]
    async fn registry_cancel_returns_true_for_existing() {
        let reg = ThreadRuntimeRegistry::new();
        reg.get_or_create("acme", "t1");
        assert!(reg.cancel("acme", "t1"));
    }

    #[test]
    fn gc_removes_idle_runtimes() {
        let reg = ThreadRuntimeRegistry {
            runtimes: DashMap::new(),
        };
        let rt = ThreadRuntime::new("acme", "t1");
        // Force idle by setting last_activity far in the past.
        rt.last_activity
            .store(now_secs() - IDLE_TTL_SECS - 1, Ordering::Relaxed);
        reg.runtimes.insert(("acme".into(), "t1".into()), rt);
        assert_eq!(reg.len(), 1);
        reg.gc();
        assert_eq!(reg.len(), 0, "idle runtime should be evicted");
    }
}
