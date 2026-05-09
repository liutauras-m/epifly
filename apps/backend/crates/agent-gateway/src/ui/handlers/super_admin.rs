//! Super-admin UI handlers for capability management.

use crate::state::AppState;
use crate::ui::session::SessionUser;
use agent_core::{
    CapabilitySummary, CreateCapabilityRequest, UpdateCapabilityRequest,
};
use askama::Template;
use axum::{
    Form,
    extract::{Path, State},
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ── Template views ────────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "super_admin/list.html")]
pub struct AdminListView {
    pub title: &'static str,
    pub user_name: String,
    pub user_initials: String,
    pub capabilities: Vec<CapabilitySummary>,
    pub flash: Option<String>,
}

#[derive(Template)]
#[template(path = "super_admin/new.html")]
pub struct AdminNewView {
    pub title: &'static str,
    pub user_name: String,
    pub user_initials: String,
    pub error: Option<String>,
    pub manifest_toml: String,
}

#[derive(Template)]
#[template(path = "super_admin/detail.html")]
pub struct AdminDetailView {
    pub title: &'static str,
    pub user_name: String,
    pub user_initials: String,
    pub capability: CapabilitySummary,
    pub manifest_toml: String,
    pub error: Option<String>,
    pub flash: Option<String>,
}

// ── Form types ────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateForm {
    pub manifest_toml: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateForm {
    pub manifest_toml: String,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

pub async fn index(
    State(state): State<Arc<AppState>>,
    user: SessionUser,
) -> Response {
    let capabilities = state.tool_admin.list();
    let view = AdminListView {
        title: "Super Admin",
        user_name: user.name.clone(),
        user_initials: user.initials(),
        capabilities,
        flash: None,
    };
    Html(view.render().unwrap_or_else(|e| format!("<pre>{e}</pre>"))).into_response()
}

pub async fn new_form(
    _state: State<Arc<AppState>>,
    user: SessionUser,
) -> Response {
    let default_toml = r#"name = "my-capability"
version = "0.1.0"
description = "Describe what this capability does."
kind = "chain"
tags = []
tools = []

[chain]
model = "claude-opus-4-7"
system_prompt = "You are a helpful assistant."
prompt_template = "{{input.query}}"
max_tokens = 2048
"#;
    let view = AdminNewView {
        title: "New Capability",
        user_name: user.name.clone(),
        user_initials: user.initials(),
        error: None,
        manifest_toml: default_toml.to_string(),
    };
    Html(view.render().unwrap_or_else(|e| format!("<pre>{e}</pre>"))).into_response()
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    user: SessionUser,
    Form(form): Form<CreateForm>,
) -> Response {
    let tenant = user.tenant_context();
    let req = CreateCapabilityRequest {
        manifest_toml: form.manifest_toml.clone(),
        wasm_bytes: None,
    };
    match state.tool_admin.create(req, &tenant) {
        Ok(summary) => Redirect::to(&format!("/super-admin/{}", summary.name)).into_response(),
        Err(e) => {
            let view = AdminNewView {
                title: "New Capability",
                user_name: user.name.clone(),
                user_initials: user.initials(),
                error: Some(e.to_string()),
                manifest_toml: form.manifest_toml,
            };
            Html(view.render().unwrap_or_else(|e2| format!("<pre>{e2}</pre>"))).into_response()
        }
    }
}

pub async fn detail(
    State(state): State<Arc<AppState>>,
    user: SessionUser,
    Path(name): Path<String>,
) -> Response {
    let Some(capability) = state.tool_admin.get(&name) else {
        return (axum::http::StatusCode::NOT_FOUND, "Capability not found").into_response();
    };
    let manifest_toml = state.tool_admin.get_manifest_toml(&name).unwrap_or_default();
    let view = AdminDetailView {
        title: "Capability Detail",
        user_name: user.name.clone(),
        user_initials: user.initials(),
        capability,
        manifest_toml,
        error: None,
        flash: None,
    };
    Html(view.render().unwrap_or_else(|e| format!("<pre>{e}</pre>"))).into_response()
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    user: SessionUser,
    Path(name): Path<String>,
    Form(form): Form<UpdateForm>,
) -> Response {
    let tenant = user.tenant_context();
    let req = UpdateCapabilityRequest { manifest_toml: form.manifest_toml.clone() };
    match state.tool_admin.update(&name, req, &tenant) {
        Ok(capability) => {
            let manifest_toml = state.tool_admin.get_manifest_toml(&name).unwrap_or_default();
            let view = AdminDetailView {
                title: "Capability Detail",
                user_name: user.name.clone(),
                user_initials: user.initials(),
                capability,
                manifest_toml,
                error: None,
                flash: Some("Capability updated successfully.".into()),
            };
            Html(view.render().unwrap_or_else(|e| format!("<pre>{e}</pre>"))).into_response()
        }
        Err(e) => {
            let capability = state.tool_admin.get(&name).unwrap_or_else(|| CapabilitySummary {
                name: name.clone(),
                version: "?".into(),
                description: String::new(),
                kind: "?".into(),
                enabled: false,
                tags: vec![],
                last_error: None,
                registered_at: String::new(),
                updated_at: String::new(),
            });
            let view = AdminDetailView {
                title: "Capability Detail",
                user_name: user.name.clone(),
                user_initials: user.initials(),
                capability,
                manifest_toml: form.manifest_toml,
                error: Some(e.to_string()),
                flash: None,
            };
            Html(view.render().unwrap_or_else(|e2| format!("<pre>{e2}</pre>"))).into_response()
        }
    }
}

pub async fn toggle_enabled(
    State(state): State<Arc<AppState>>,
    user: SessionUser,
    Path(name): Path<String>,
) -> Response {
    let tenant = user.tenant_context();
    // Toggle current state.
    let current = state.tool_admin.get(&name).map(|c| c.enabled).unwrap_or(false);
    let _ = state.tool_admin.set_enabled(&name, !current, &tenant);
    Redirect::to(&format!("/super-admin/{name}")).into_response()
}

pub async fn delete_cap(
    State(state): State<Arc<AppState>>,
    user: SessionUser,
    Path(name): Path<String>,
) -> Response {
    let tenant = user.tenant_context();
    let _ = state.tool_admin.delete(&name, &tenant);
    Redirect::to("/super-admin").into_response()
}

pub async fn reload_cap(
    State(state): State<Arc<AppState>>,
    user: SessionUser,
    Path(name): Path<String>,
) -> Response {
    let tenant = user.tenant_context();
    let _ = state.tool_admin.reload(&name, &tenant);
    Redirect::to(&format!("/super-admin/{name}")).into_response()
}

pub async fn reload_all_caps(
    State(state): State<Arc<AppState>>,
    user: SessionUser,
) -> Response {
    let tenant = user.tenant_context();
    let _ = state.tool_admin.reload_all(&tenant);
    Redirect::to("/super-admin").into_response()
}
