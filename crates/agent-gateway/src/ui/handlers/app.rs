//! App shell handler — renders the full sidebar + greeting + composer.

use crate::state::AppState;
use crate::ui::session::SessionUser;
use crate::ui::view::{AppView, CapView, RecentView, glyph_for, time_greeting};
use askama::Template;
use axum::{
    extract::State,
    response::{Html, IntoResponse, Response},
};
use std::sync::Arc;

pub async fn index(State(state): State<Arc<AppState>>, user: SessionUser) -> Response {
    let tenant = user.tenant_context();
    let recents = state
        .thread_store
        .list(&tenant.tenant_id, 20)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|t| RecentView {
            id: t.id.clone(),
            title: t.title.unwrap_or_else(|| "Untitled thread".into()),
        })
        .collect();

    let capabilities: Vec<CapView> = {
        let registry = state.registry.lock().unwrap();
        registry
            .all()
            .map(|c| CapView {
                name: c.manifest.name.clone(),
                kind_glyph: glyph_for(&format!("{:?}", c.manifest.kind)).to_string(),
                tool_count: c.manifest.tools.len(),
            })
            .collect()
    };

    let view = AppView {
        title: "Workshop",
        year: chrono::Utc::now()
            .format("%Y")
            .to_string()
            .parse()
            .unwrap_or(2026),
        user_first_name: user.first_name().to_string(),
        user_initials: user.initials(),
        user_plan: user.plan.to_uppercase(),
        user_name: user.name.clone(),
        greeting: time_greeting(),
        recents,
        capabilities,
    };

    Html(view.render().unwrap_or_else(|e| format!("<pre>{e}</pre>"))).into_response()
}
