use axum::{extract::State, Extension, Json};
use serde_json::{json, Value};
use std::sync::Arc;
use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;

pub async fn list_capabilities(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
) -> Json<Value> {
    let registry = state.registry.lock().unwrap();
    let caps: Vec<Value> = registry.all().map(|card| {
        json!({
            "name": card.manifest.name,
            "version": card.manifest.version,
            "description": card.manifest.description,
            "kind": format!("{:?}", card.manifest.kind),
            "tags": card.manifest.tags,
            "tools": card.manifest.tools.iter().map(|t| json!({
                "name": t.name,
                "description": t.description,
            })).collect::<Vec<_>>(),
        })
    }).collect();
    Json(json!({
        "tenant_id": tenant.0.tenant_id,
        "plan": tenant.0.plan.to_string(),
        "capabilities": caps,
    }))
}
