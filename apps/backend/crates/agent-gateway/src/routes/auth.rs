use axum::response::{IntoResponse, Redirect};

fn web_login_url() -> String {
    let raw = std::env::var("WEB_ORIGIN").unwrap_or_else(|_| {
        "http://localhost:3000,http://localhost:5173,https://tauri.localhost,tauri://localhost"
            .into()
    });

    let origin = raw
        .split(',')
        .find_map(|value| {
            let trimmed = value.trim();
            (!trimmed.is_empty()).then(|| trimmed.trim_end_matches('/'))
        })
        .unwrap_or("http://localhost:3000");

    format!("{origin}/login")
}

/// `GET /login` — public entrypoint that sends the browser to the SvelteKit login page.
pub async fn login_page() -> impl IntoResponse {
    Redirect::to(&web_login_url())
}
