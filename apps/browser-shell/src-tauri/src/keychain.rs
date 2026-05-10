/// In-process device token state.
///
/// The token is provisioned once (by an admin) and stored in Stronghold via
/// the JS frontend (`@tauri-apps/plugin-stronghold`). On each startup the
/// frontend loads it from Stronghold and calls `set_device_token` so Rust can
/// use it for WS connections and capability registration without re-reading
/// Stronghold on every request.
use std::sync::{Arc, Mutex};

pub struct DeviceTokenState(pub Option<String>);
pub type DeviceTokenHandle = Arc<Mutex<DeviceTokenState>>;

#[tauri::command]
pub fn set_device_token(state: tauri::State<DeviceTokenHandle>, token: String) {
    state.lock().unwrap().0 = Some(token);
}

#[tauri::command]
pub fn get_device_token(state: tauri::State<DeviceTokenHandle>) -> Option<String> {
    state.lock().unwrap().0.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let state = Arc::new(Mutex::new(DeviceTokenState(None)));
        {
            let mut guard = state.lock().unwrap();
            guard.0 = Some("tok_abc".to_owned());
        }
        let got = state.lock().unwrap().0.clone();
        assert_eq!(got, Some("tok_abc".to_owned()));
    }
}
