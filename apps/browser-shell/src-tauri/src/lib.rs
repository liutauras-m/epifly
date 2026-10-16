mod auth;
mod chat_stream;
mod recorder;
mod registration;
mod tabs;
pub mod telemetry;

use auth::AuthState;
use chat_stream::StreamRegistry;
use recorder::RecorderStateHandle;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tabs::TabManagerState;

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let tabs_state: TabManagerState = Arc::new(Mutex::new(tabs::TabManager::default()));
    let recorder_state: RecorderStateHandle = Arc::new(Mutex::new(recorder::RecorderState::new()));

    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default();

    // WebDriver automation server — debug builds, macOS, with `e2e` feature only.
    // Exposes a W3C WebDriver endpoint so WebdriverIO can drive the WKWebView.
    #[cfg(all(debug_assertions, target_os = "macos", feature = "e2e"))]
    {
        builder = builder.plugin(tauri_plugin_webdriver_automation::init());
    }

    builder
        .plugin(tauri_plugin_dialog::init())
        // No Stronghold plugin and no HTTP plugin: the refresh-token vault is
        // used from Rust only (auth/vault.rs) and gateway calls are plain
        // fetch() under the CSP connect-src, so no webview gets either API.
        .plugin(tauri_plugin_haptics::init())
        .manage(tabs_state)
        .manage(recorder_state)
        .manage(AuthState::new())
        .manage(Arc::new(Mutex::new(
            HashMap::<String, tokio::task::JoinHandle<()>>::new(),
        )) as StreamRegistry)
        .invoke_handler(tauri::generate_handler![
            tabs::create_tab,
            tabs::close_tab,
            tabs::navigate_tab,
            tabs::list_tabs,
            tabs::save_tabs,
            tabs::restore_tabs,
            recorder::recorder_start,
            recorder::recorder_record_step,
            recorder::recorder_stop,
            recorder::recorder_status,
            registration::upload_trace_cmd,
            chat_stream::chat_stream_start,
            chat_stream::chat_stream_abort,
            auth::auth_status,
            auth::auth_sign_in,
            auth::auth_sign_out,
            auth::auth_access_token,
        ])
        .setup(|_app| {
            tracing::info!(gateway = ?auth::gateway_url(), "browser-shell started");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error running ConusAI Browser");
}

pub fn recorder_bridge_js() -> &'static str {
    RECORDER_BRIDGE_JS
}
