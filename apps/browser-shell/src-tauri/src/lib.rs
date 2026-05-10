mod keychain;
mod recorder;
mod registration;
mod tabs;

use keychain::DeviceTokenHandle;
use recorder::RecorderStateHandle;
use std::sync::{Arc, Mutex};
use tabs::TabsState;
use tauri::{Emitter, Manager};

// JS injected into every child webview (tab content) to forward DOM events to
// the Tauri recorder. The main shell window handles its own events via Svelte.
const RECORDER_BRIDGE_JS: &str = r#"
(function () {
  if (window.__conusai_bridge_installed__) return;
  window.__conusai_bridge_installed__ = true;

  const invoke = window.__TAURI__?.core?.invoke;
  if (!invoke) return;

  function send(kind, extra) {
    invoke('recorder_record_step', {
      step: { kind, url: location.href, timestamp_ms: Date.now(), ...extra }
    }).catch(() => {});
  }

  document.addEventListener('click', function (e) {
    const el = e.target.closest('a,button,[role=button]');
    if (!el) return;
    const sel = el.id ? '#' + el.id : el.tagName.toLowerCase();
    send('click', { selector: sel });
  }, { capture: true, passive: true });

  document.addEventListener('change', function (e) {
    const el = e.target;
    if (!el || !el.name) return;
    const isPii = /password|ssn|cc-|card|cvv/i.test(el.name + ' ' + (el.id || ''));
    send('input', { selector: '#' + (el.id || el.name), value: isPii ? null : el.value });
  }, { capture: true, passive: true });

  document.addEventListener('submit', function (e) {
    const form = e.target;
    send('submit', { selector: form.id ? '#' + form.id : 'form' });
  }, { capture: true, passive: true });
})();
"#;

pub fn run() {
    let tabs_state: TabsState = Arc::new(Mutex::new(tabs::Tabs::default()));
    let recorder_state: RecorderStateHandle =
        Arc::new(Mutex::new(recorder::RecorderState::new()));
    let token_state: DeviceTokenHandle = Arc::new(Mutex::new(keychain::DeviceTokenState(
        // Allow bootstrap via env var; frontend will overwrite from Stronghold once loaded.
        std::env::var("CONUSAI_DEVICE_TOKEN").ok(),
    )));

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_stronghold::Builder::new(|password| {
                blake3::hash(password.as_bytes()).as_bytes().to_vec()
            })
            .build(),
        )
        .plugin(tauri_plugin_http::init())
        .manage(tabs_state)
        .manage(recorder_state)
        .manage(token_state)
        .invoke_handler(tauri::generate_handler![
            tabs::create_tab,
            tabs::close_tab,
            tabs::navigate_tab,
            tabs::list_tabs,
            recorder::recorder_start,
            recorder::recorder_record_step,
            recorder::recorder_stop,
            recorder::recorder_status,
            keychain::set_device_token,
            keychain::get_device_token,
        ])
        .setup(|app| {
            let api_base = std::env::var("CONUSAI_API_BASE")
                .unwrap_or_else(|_| "http://localhost:8080".to_owned());
            let token_handle = app.state::<DeviceTokenHandle>().inner().clone();
            let app_handle = app.handle().clone();

            tauri::async_runtime::spawn(async move {
                // Emit shell-ready so the frontend can load the token from Stronghold
                // and call set_device_token, after which registration fires.
                if let Some(win) = app_handle.get_webview_window("main") {
                    let _ = win.emit("shell-ready", ());
                }

                // Give the frontend a moment to load the token from Stronghold.
                tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

                let token = token_handle.lock().unwrap().0.clone();
                if let Some(token) = token {
                    if let Err(e) =
                        registration::register_capability(&api_base, &token).await
                    {
                        tracing::warn!(error = %e, "capability registration failed");
                    }
                } else {
                    tracing::warn!("no device token available — skipping capability registration");
                }

                tracing::info!(api_base, "browser-shell started");
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error running ConusAI Browser");
}

pub fn recorder_bridge_js() -> &'static str {
    RECORDER_BRIDGE_JS
}
