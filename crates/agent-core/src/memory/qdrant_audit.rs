/// QdrantAuditStore — append-only audit log backed by Qdrant.
///
/// Data layout (per tenant, collection `audit_{tenant_id}`):
///   • Each audit event is one Qdrant point with a zero-vector and full payload.
///   • Point ID is derived from the event ULID (deterministic, monotonic-friendly).
///   • Retrieval is payload-filtered and ordered by `timestamp` descending.
use super::qdrant_helpers::{QdrantClient, point_id, zero_vec};
use async_trait::async_trait;
use common::audit::{AuditEvent, AuditStore};
use serde_json::json;
use tracing::{instrument, warn};

pub struct QdrantAuditStore {
    qdrant: QdrantClient,
}

impl QdrantAuditStore {
    pub fn new(qdrant_url: impl Into<String>) -> Self {
        Self {
            qdrant: QdrantClient::new(qdrant_url),
        }
    }

    fn collection(&self, tenant_id: &str) -> String {
        format!("audit_{tenant_id}")
    }

    async fn ensure_collection(&self, tenant_id: &str) -> common::error::Result<()> {
        let col = self.collection(tenant_id);
        self.qdrant
            .ensure_collection(&col, &[], &[])
            .await
            .map_err(|e| common::error::ConusAiError::Storage(e.to_string()))
    }
}

#[async_trait]
impl AuditStore for QdrantAuditStore {
    #[instrument(skip(self, event), fields(tenant_id = %event.tenant_id, action = %event.action))]
    async fn append(&self, event: AuditEvent) -> common::error::Result<()> {
        if let Err(e) = self.ensure_collection(&event.tenant_id).await {
            warn!("audit: failed to ensure collection: {e}");
            return Err(e);
        }

        let col = self.collection(&event.tenant_id);
        let payload: serde_json::Value = serde_json::to_value(&event)
            .map_err(|e| common::error::ConusAiError::Storage(e.to_string()))?;

        let pid = point_id(&event.id);
        let point = json!({
            "id": pid,
            "vector": zero_vec(),
            "payload": payload,
        });

        self.qdrant
            .upsert_point(&col, point)
            .await
            .map_err(|e| common::error::ConusAiError::Storage(e.to_string()))
    }

    #[instrument(skip(self), fields(tenant_id, limit))]
    async fn list(&self, tenant_id: &str, limit: usize) -> common::error::Result<Vec<AuditEvent>> {
        self.ensure_collection(tenant_id).await?;

        let col = self.collection(tenant_id);
        let url = format!("{}/collections/{}/points/scroll", self.qdrant.base_url, col);

        // Audit list uses `order_by` which isn't part of the generic scroll_filter helper,
        // so we call the REST endpoint directly here.
        let client = reqwest::Client::new();
        let body = json!({
            "limit": limit,
            "with_payload": true,
            "with_vector": false,
            "order_by": { "key": "timestamp", "direction": "desc" }
        });

        let resp: serde_json::Value = client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| common::error::ConusAiError::Storage(e.to_string()))?
            .json()
            .await
            .map_err(|e| common::error::ConusAiError::Storage(e.to_string()))?;

        let points = resp["result"]["points"]
            .as_array()
            .cloned()
            .unwrap_or_default();

        let mut events = Vec::with_capacity(points.len());
        for p in points {
            if let Ok(ev) = serde_json::from_value::<AuditEvent>(p["payload"].clone()) {
                events.push(ev);
            }
        }

        Ok(events)
    }
}
