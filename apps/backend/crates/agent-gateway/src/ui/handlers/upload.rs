//! UI file upload — multipart → RustFS, returns object key for the composer chip.
//! No in-memory token map — the object key is the durable reference.
//!
//! After a successful upload:
//! 1. The attachment is registered in the workspace via the `storage-workspace` capability.
//! 2. A `workspace.uploaded` realtime event is broadcast so the UI can update without polling.
//! 3. If a `plan-on-upload` capability is registered for the tenant, its plan steps are
//!    resolved and executed in a background task.

use crate::state::AppState;
use crate::ui::session::SessionUser;
use agent_core::realtime::WorkspaceChangeEvent;
use agent_core::{PlanStep, run_plan};
use axum::{
    Json,
    extract::{Multipart, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use object_store::{ObjectStore, path::Path as OsPath};
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::warn;
use uuid::Uuid;

pub async fn ui_upload(
    State(state): State<Arc<AppState>>,
    user: SessionUser,
    mut multipart: Multipart,
) -> Response {
    let store = match state.file_store.as_ref() {
        Some(s) => s,
        None => {
            return err(
                StatusCode::SERVICE_UNAVAILABLE,
                "file storage not configured",
            );
        }
    };

    let tenant = user.tenant_context();

    let storage_factory = match state.tenant_storage.as_ref() {
        Some(f) => f,
        None => return err(StatusCode::SERVICE_UNAVAILABLE, "storage not configured"),
    };
    let storage = match storage_factory.for_tenant(tenant.tenant_id.as_str()).await {
        Ok(s) => s,
        Err(e) => return err(StatusCode::INTERNAL_SERVER_ERROR, &format!("storage: {e}")),
    };

    let field = match multipart.next_field().await {
        Ok(Some(f)) => f,
        Ok(None) => return err(StatusCode::BAD_REQUEST, "no file in upload"),
        Err(e) => return err(StatusCode::BAD_REQUEST, &format!("multipart: {e}")),
    };

    let filename = field
        .file_name()
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{}.bin", Uuid::new_v4()));
    let content_type = field
        .content_type()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "application/octet-stream".into());

    let data = match field.bytes().await {
        Ok(b) => b,
        Err(e) => return err(StatusCode::BAD_REQUEST, &format!("read: {e}")),
    };
    let size = data.len();

    let object_key = storage.attachment_s3_key(&Uuid::new_v4().to_string(), &filename);
    let os_path = OsPath::from(object_key.as_str());

    if let Err(e) = store.put(&os_path, data.into()).await {
        warn!(error = %e, "ui upload write failed");
        return err(StatusCode::INTERNAL_SERVER_ERROR, &format!("storage: {e}"));
    }

    // 6.1 — Register attachment as a workspace node via the storage-workspace capability.
    // Clone the Arc under a short-lived lock, then invoke without holding the mutex.
    {
        let provider = {
            let reg = state.registry.read();
            reg.get_provider("storage-workspace")
        };
        if let Some(prov) = provider {
            let meta_content = format!(
                "<!-- attachment -->\n- **file**: {filename}\n- **key**: {object_key}\n- **size**: {size} bytes\n- **type**: {content_type}\n"
            );
            let base_name = filename.trim_end_matches(".md");
            let attach_input = json!({
                "folder_name": "Uploads",
                "filename": base_name,
                "content": meta_content,
            });
            if let Err(e) = prov
                .invoke("save_document", &attach_input, Some(&tenant))
                .await
            {
                warn!(error = %e, "upload: workspace registration failed (non-fatal)");
            }
        }
    }

    // 6.2 — Per-tenant on_upload policy: if a plan-on-upload capability is registered,
    // resolve its plan steps and execute them in a background task.
    {
        let provider = {
            let reg = state.registry.read();
            reg.get_provider("plan-on-upload")
        };
        if let Some(prov) = provider {
            let trigger_input = json!({
                "object_key": &object_key,
                "filename": &filename,
                "content_type": &content_type,
                "size": size,
            });
            match prov.invoke("plan", &trigger_input, Some(&tenant)).await {
                Ok(steps_val) => {
                    if let Ok(steps) = serde_json::from_value::<Vec<PlanStep>>(steps_val) {
                        let registry = Arc::clone(&state.registry);
                        let llm = Arc::clone(&state.llm);
                        let realtime = Arc::clone(&state.realtime_service);
                        let tenant_bg = tenant.clone();
                        tokio::spawn(async move {
                            run_plan(steps, registry, Some(llm), Some(tenant_bg), Some(realtime))
                                .await;
                        });
                    }
                }
                Err(e) => warn!(error = %e, "plan-on-upload: step resolution failed (non-fatal)"),
            }
        }
    }

    // Emit workspace.uploaded realtime event so UI can update without polling.
    let attachment_id = Uuid::new_v4().to_string();
    state
        .realtime_service
        .publish_workspace_change(WorkspaceChangeEvent {
            op: "workspace.uploaded".into(),
            tenant_id: tenant.tenant_id.to_string(),
            node_id: attachment_id.clone(),
            kind: "attachment".into(),
        })
        .await;

    let payload: Value = json!({
        "id": object_key,
        "attachment_id": attachment_id,
        "filename": filename,
        "size": size,
        "content_type": content_type,
        "download_url": format!("/ui/files/download?key={}", urlencoding::encode(&object_key)),
    });
    (StatusCode::OK, Json(payload)).into_response()
}

fn err(code: StatusCode, msg: &str) -> Response {
    (code, Json(json!({ "error": msg }))).into_response()
}

fn urlencoding_encode(s: &str) -> String {
    s.chars()
        .flat_map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '~' || c == '/' {
                vec![c]
            } else {
                format!("%{:02X}", c as u32).chars().collect()
            }
        })
        .collect()
}

mod urlencoding {
    pub fn encode(s: &str) -> String {
        super::urlencoding_encode(s)
    }
}
