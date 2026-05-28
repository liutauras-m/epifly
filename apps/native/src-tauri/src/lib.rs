mod auth;

use auth::{auth_get_access_token, auth_sign_out, auth_start, AuthState};
use tauri_plugin_deep_link::DeepLinkExt;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AuthState::default())
        .setup(|app| {
            let handle = app.handle().clone();

            // Cold-start: handle URL the app was launched with (desktop platforms)
            #[cfg(not(target_os = "ios"))]
            {
                if let Ok(Some(urls)) = app.deep_link().get_current() {
                    for u in urls {
                        let h = handle.clone();
                        tauri::async_runtime::spawn(async move {
                            auth::handle_callback_url(&h, u).await;
                        });
                    }
                }
            }

            // Runtime: handle URLs while the app is already running
            {
                let h = handle.clone();
                app.deep_link().on_open_url(move |event| {
                    for u in event.urls() {
                        let h2 = h.clone();
                        let url = u.clone();
                        tauri::async_runtime::spawn(async move {
                            auth::handle_callback_url(&h2, url).await;
                        });
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            auth_start,
            auth_get_access_token,
            auth_sign_out,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Epifly");
}
