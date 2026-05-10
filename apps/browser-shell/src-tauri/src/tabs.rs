use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use ulid::Ulid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TabSummary {
    pub id: String,
    pub url: String,
    pub title: String,
}

#[derive(Default)]
pub struct TabManager {
    inner: HashMap<String, TabSummary>,
}

pub type TabManagerState = Arc<Mutex<TabManager>>;

impl TabManager {
    pub fn create(&mut self, app: &AppHandle, url: &str) -> anyhow::Result<String> {
        let id = Ulid::new().to_string();
        let label = format!("tab-{id}");

        let data_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| anyhow::anyhow!("app_data_dir: {e}"))?
            .join("tabs")
            .join(&label);

        WebviewWindowBuilder::new(app, &label, WebviewUrl::External(url.parse()?))
            .title("ConusAI Browser")
            .initialization_script(crate::recorder_bridge_js())
            .data_directory(data_dir)
            .build()?;

        self.inner.insert(
            id.clone(),
            TabSummary {
                id: id.clone(),
                url: url.to_owned(),
                title: label,
            },
        );
        Ok(id)
    }

    pub fn close(&mut self, app: &AppHandle, id: &str) {
        if self.inner.remove(id).is_some() {
            if let Some(w) = app.get_webview_window(&format!("tab-{id}")) {
                let _ = w.close();
            }
        }
    }

    pub fn navigate(&mut self, app: &AppHandle, id: &str, url: &str) -> anyhow::Result<()> {
        if let Some(tab) = self.inner.get_mut(id) {
            if let Some(w) = app.get_webview_window(&format!("tab-{id}")) {
                w.navigate(url.parse()?)?;
                tab.url = url.to_owned();
            }
        }
        Ok(())
    }

    pub fn list(&self) -> Vec<TabSummary> {
        self.inner.values().cloned().collect()
    }
}

#[tauri::command]
pub fn create_tab(
    app: AppHandle,
    state: tauri::State<TabManagerState>,
    url: String,
) -> Result<String, String> {
    state
        .lock()
        .unwrap()
        .create(&app, &url)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn close_tab(app: AppHandle, state: tauri::State<TabManagerState>, id: String) {
    state.lock().unwrap().close(&app, &id);
}

#[tauri::command]
pub fn navigate_tab(
    app: AppHandle,
    state: tauri::State<TabManagerState>,
    id: String,
    url: String,
) -> Result<(), String> {
    state
        .lock()
        .unwrap()
        .navigate(&app, &id, &url)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_tabs(state: tauri::State<TabManagerState>) -> Vec<TabSummary> {
    state.lock().unwrap().list()
}

/// Serialize the current tab list to `$APP_DATA/tabs.json`.
#[tauri::command]
pub fn save_tabs(
    app: AppHandle,
    state: tauri::State<TabManagerState>,
) -> Result<(), String> {
    let summaries = state.lock().unwrap().list();
    let json = serde_json::to_string(&summaries).map_err(|e| e.to_string())?;
    let path = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("tabs.json");
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

/// Read `$APP_DATA/tabs.json` and return the persisted tab summaries.
/// The JS caller is responsible for recreating tabs via `create_tab`.
#[tauri::command]
pub fn restore_tabs(app: AppHandle) -> Result<Vec<TabSummary>, String> {
    let path = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("tabs.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let data = std::fs::read(&path).map_err(|e| e.to_string())?;
    serde_json::from_slice(&data).map_err(|e| e.to_string())
}
