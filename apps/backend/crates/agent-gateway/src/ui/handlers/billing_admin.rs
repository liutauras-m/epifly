//! Super-admin billing dashboard — server-rendered HTML page.
//! GET /ui/admin/billing

use crate::state::AppState;
use axum::{
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use std::sync::Arc;
use tracing::warn;

pub async fn billing_admin_dashboard(State(state): State<Arc<AppState>>) -> Response {
    let billing = match state.billing.as_ref() {
        Some(b) => b,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                html("<h1>Billing not configured</h1><p>Set <code>LAGO_API_KEY</code> to enable billing.</p>"),
            )
                .into_response();
        }
    };

    let summary = match billing.analytics_summary().await {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, "billing analytics_summary failed");
            serde_json::json!({ "error": e.to_string() })
        }
    };

    let summary_pretty = serde_json::to_string_pretty(&summary)
        .unwrap_or_else(|_| "{}".into());

    let page = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Billing Admin — ConusAI</title>
  <style>
    body {{ font-family: system-ui, sans-serif; max-width: 900px; margin: 2rem auto; padding: 0 1rem; color: #1a1a2e; }}
    h1 {{ color: #16213e; }}
    pre {{ background: #f4f4f8; padding: 1rem; border-radius: 8px; overflow-x: auto; font-size: 0.85rem; }}
    .section {{ margin-top: 2rem; }}
    .badge {{ display: inline-block; padding: 2px 8px; border-radius: 4px; font-size: 0.75rem;
              font-weight: 600; background: #e0e7ff; color: #3730a3; }}
  </style>
</head>
<body>
  <h1>Billing Admin</h1>
  <p><span class="badge">Lago</span> analytics summary</p>
  <div class="section">
    <h2>Analytics</h2>
    <pre>{summary_pretty}</pre>
  </div>
  <div class="section">
    <h2>Actions</h2>
    <p>Use the API to manage subscriptions and credits:</p>
    <ul>
      <li><code>POST /admin/billing/credits</code> — add wallet credits</li>
      <li><code>POST /admin/billing/cancel/{{tenant_id}}</code> — cancel subscription</li>
      <li><code>GET  /admin/billing/dashboard</code> — JSON analytics (API)</li>
    </ul>
  </div>
</body>
</html>"#
    );

    (StatusCode::OK, html(page)).into_response()
}

fn html(body: impl Into<String>) -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        body.into(),
    )
}
