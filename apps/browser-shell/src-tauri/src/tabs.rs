use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use ulid::Ulid;

#[derive(Debug, Clone, serde::Serialize)]
pub struct TabSummary {
    pub id: String,
    pub url: String,
    pub title: String,
}

#[derive(Default)]
pub struct Tabs {
    inner: HashMap<String, TabSummary>,
}

pub type TabsState = Arc<Mutex<Tabs>>;

impl Tabs {
    pub fn create(&mut self, app: &AppHandle, url: &str) -> anyhow::Result<String> {
        let id = Ulid::new().to_string();
        let label = format!("tab-{id}");

        WebviewWindowBuilder::new(app, &label, WebviewUrl::External(url.parse()?))
            .title("ConusAI Browser")
            .initialization_script(crate::recorder_bridge_js())
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
    state: tauri::State<TabsState>,
    url: String,
) -> Result<String, String> {
    state
        .lock()
        .unwrap()
        .create(&app, &url)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn close_tab(app: AppHandle, state: tauri::State<TabsState>, id: String) {
    state.lock().unwrap().close(&app, &id);
}

#[tauri::command]
pub fn navigate_tab(
    app: AppHandle,
    state: tauri::State<TabsState>,
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
pub fn list_tabs(state: tauri::State<TabsState>) -> Vec<TabSummary> {
    state.lock().unwrap().list()
}
