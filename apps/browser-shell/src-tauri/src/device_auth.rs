/// Device authentication — in-process token state with trait abstractions.
///
/// The device token is provisioned once by an admin and persisted in Stronghold
/// via the JS frontend. On startup the frontend reads it from Stronghold and
/// calls `set_device_token` so Rust can use it for WS connections and
/// capability registration without re-reading Stronghold per request.
use std::sync::{Arc, RwLock};

// ── Traits ───────────────────────────────────────────────────────────────────

/// Read-only access to the in-flight device token.
pub trait DeviceTokenProvider: Send + Sync {
    fn token(&self) -> Option<String>;
}

/// Write access used by privileged callers (admin panel, JS bridge).
pub trait DeviceAuthAdmin: DeviceTokenProvider {
    fn set_token(&self, token: String);
    fn clear_token(&self);
}

// ── Concrete service ──────────────────────────────────────────────────────────

pub struct DeviceAuthService {
    inner: Arc<RwLock<Option<String>>>,
}

impl DeviceAuthService {
    pub fn new(initial: Option<String>) -> Self {
        Self { inner: Arc::new(RwLock::new(initial)) }
    }
}

impl DeviceTokenProvider for DeviceAuthService {
    fn token(&self) -> Option<String> {
        self.inner.read().unwrap().clone()
    }
}

impl DeviceAuthAdmin for DeviceAuthService {
    fn set_token(&self, token: String) {
        *self.inner.write().unwrap() = Some(token);
    }

    fn clear_token(&self) {
        *self.inner.write().unwrap() = None;
    }
}

// Shared handle used as Tauri managed state.
pub type DeviceAuthHandle = Arc<DeviceAuthService>;

// ── Tauri commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn set_device_token(state: tauri::State<DeviceAuthHandle>, token: String) {
    state.set_token(token);
}

#[tauri::command]
pub fn get_device_token(state: tauri::State<DeviceAuthHandle>) -> Option<String> {
    state.token()
}

#[tauri::command]
pub fn clear_device_token(state: tauri::State<DeviceAuthHandle>) {
    state.clear_token();
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn svc(init: Option<&str>) -> Arc<DeviceAuthService> {
        Arc::new(DeviceAuthService::new(init.map(|s| s.to_owned())))
    }

    #[test]
    fn starts_empty_by_default() {
        let s = svc(None);
        assert!(s.token().is_none());
    }

    #[test]
    fn starts_with_bootstrap_token() {
        let s = svc(Some("tok_bootstrap"));
        assert_eq!(s.token(), Some("tok_bootstrap".to_owned()));
    }

    #[test]
    fn set_then_get_round_trip() {
        let s = svc(None);
        s.set_token("tok_abc".to_owned());
        assert_eq!(s.token(), Some("tok_abc".to_owned()));
    }

    #[test]
    fn clear_removes_token() {
        let s = svc(Some("tok_xyz"));
        s.clear_token();
        assert!(s.token().is_none());
    }

    #[test]
    fn overwrite_updates_token() {
        let s = svc(Some("tok_old"));
        s.set_token("tok_new".to_owned());
        assert_eq!(s.token(), Some("tok_new".to_owned()));
    }

    #[test]
    fn concurrent_reads_are_consistent() {
        use std::thread;
        let s = svc(Some("tok_concurrent"));
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let clone = Arc::clone(&s);
                thread::spawn(move || clone.token())
            })
            .collect();
        for h in handles {
            assert_eq!(h.join().unwrap(), Some("tok_concurrent".to_owned()));
        }
    }

    #[test]
    fn mock_provider_satisfies_trait() {
        struct MockProvider(Option<String>);
        impl DeviceTokenProvider for MockProvider {
            fn token(&self) -> Option<String> { self.0.clone() }
        }

        fn needs_provider(p: &dyn DeviceTokenProvider) -> bool {
            p.token().is_some()
        }

        assert!(needs_provider(&MockProvider(Some("t".to_owned()))));
        assert!(!needs_provider(&MockProvider(None)));
    }
}
