//! Askama template structs.

use askama::Template;

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginView {
    pub title: &'static str,
    pub year: i32,
    pub greeting_eyebrow: String,
    pub default_name: String,
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "app.html")]
pub struct AppView {
    pub title: &'static str,
    #[allow(dead_code)]
    pub year: i32,
    pub user_name: String,
    pub user_first_name: String,
    pub user_initials: String,
    pub user_plan: String,
    pub greeting: String,
    pub recents: Vec<RecentView>,
    pub capabilities: Vec<CapView>,
}

pub struct RecentView {
    pub id: String,
    pub title: String,
}

pub struct CapView {
    pub name: String,
    pub kind_glyph: String,
    pub tool_count: usize,
}

pub fn glyph_for(kind: &str) -> &'static str {
    match kind {
        "mcp" | "Mcp" => "M",
        "wasm" | "Wasm" => "W",
        "docker" | "Docker" => "D",
        "pipeline" | "Pipeline" => "P",
        "native" | "Native" => "N",
        _ => "·",
    }
}

pub fn time_greeting() -> String {
    let hour = chrono::Local::now()
        .format("%H")
        .to_string()
        .parse::<u32>()
        .unwrap_or(12);
    match hour {
        5..=11 => "Morning".into(),
        12..=17 => "Afternoon".into(),
        18..=22 => "Evening".into(),
        _ => "Late night".into(),
    }
}
