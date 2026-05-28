/// Single-flight refresh using tokio Mutex + Notify.
///
/// Concurrent `get_access_token()` calls share exactly one refresh round-trip.
/// If a refresh is in progress, callers wait on the Notify; once the refresh
/// completes they re-read from the keychain rather than making a second request.
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};

pub struct RefreshGate {
    in_flight: Mutex<bool>,
    notify: Arc<Notify>,
}

impl Default for RefreshGate {
    fn default() -> Self {
        Self {
            in_flight: Mutex::new(false),
            notify: Arc::new(Notify::new()),
        }
    }
}

impl RefreshGate {
    /// Try to become the one refresh leader.
    /// Returns `true` if this caller should perform the refresh.
    /// Returns `false` if another caller is already refreshing — the caller
    /// should wait on `wait_for_completion()` and then re-read from the keychain.
    pub async fn try_acquire(&self) -> bool {
        let mut guard = self.in_flight.lock().await;
        if *guard {
            false
        } else {
            *guard = true;
            true
        }
    }

    /// Signal that the refresh is complete (success or failure).
    pub async fn release(&self) {
        *self.in_flight.lock().await = false;
        self.notify.notify_waiters();
    }

    /// Wait until the current in-flight refresh completes.
    pub async fn wait_for_completion(&self) {
        self.notify.notified().await;
    }
}
