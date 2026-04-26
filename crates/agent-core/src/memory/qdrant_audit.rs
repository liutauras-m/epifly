/// QdrantAuditStore — append-only audit log backed by Qdrant.
///
/// Data layout (per tenant, collection `audit_{tenant_id}`):
///   • Each audit event is one Qdrant point with a zero-vector and full payload.
///   • Point ID is derived from the event ULID (deterministic, monotonic-friendly).
///   • Retrieval is payload-filtered and ordered by `timestamp` descending.
use async_trait::async_trait;
use common::audit::{AuditEvent, AuditStore};
use reqwest::Client;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tracing::{instrument, warn};

const VECTOR_DIM: usize = 4;

fn zero_vec() -> Vec<f32> {
    vec![0.0; VECTOR_DIM]
}

fn point_id(key: &str) -> u64 {
    let mut h = Sha256::new();
    h.update(key.as_bytes());
    let digest = h.finalize();
    u64::from_le_bytes(digest[..8].try_into().unwrap())
}

pub struct QdrantAuditStore {
    qdrant_url: String,
    client: Client,
}

impl QdrantAuditStore {
    pub fn new(qdrant_url: impl Into<String>) -> Self {
        Self {
            qdrant_url: qdrant_url.into(),
            client: Client::new(),
        }
    }

    fn collection(&self, tenant_id: &str) -> String {
        format!("audit_{tenant_id}")
    }

    async fn ensure_collection(&self, tenant_id: &str) -> common::error::Result<()> {
        let col = self.collection(tenant_id);
        let url = format!("{}/collections/{col}", self.qdrant_url);
        let exists = self
            .client
            .get(&url)
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false);

        if !exists {
            let create_url = format!("{}/collections/{col}", self.qdrant_url);
            let body = json!({
                "vectors": { "size": VECTOR_DIM, "distance": "Cosine" }
            });
            self.client
                .put(&create_url)
                .json(&body)
                .send()
                .await
                .map_err(|e| common::error::ConusAiError::Storage(e.to_string()))?;
        }
        Ok(())
    }
}

#[async_trait]
impl AuditStore for QdrantAuditStore {
    #[instrument(skip(self, event), fields(tenant_id = %event.tenant_id, action = %event.action))]
    async fn append(&self, event: AuditEvent) -> common::error::Result<()> {
        let col = self.collection(&event.tenant_id);
        if let Err(e) = self.ensure_collection(&event.tenant_id).await {
            warn!("audit: failed to ensure collection: {e}");
            return Err(e);
        }

        let payload: Value = serde_json::to_value(&event)
            .map_err(|e| common::error::ConusAiError::Storage(e.to_string()))?;

        let point_id = point_id(&event.id);
        let body = json!({
            "points": [{
                "id": point_id,
                "vector": zero_vec(),
                "payload": payload,
            }]
        });

        let url = format!("{}/collections/{col}/points", self.qdrant_url);
        self.client
            .put(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| common::error::ConusAiError::Storage(e.to_string()))?;

        Ok(())
    }

    #[instrument(skip(self), fields(tenant_id, limit))]
    async fn list(&self, tenant_id: &str, limit: usize) -> common::error::Result<Vec<AuditEvent>> {
        let col = self.collection(tenant_id);
        self.ensure_collection(tenant_id).await?;

        let url = format!("{}/collections/{col}/points/scroll", self.qdrant_url);
        let body = json!({
            "limit": limit,
            "with_payload": true,
            "with_vector": false,
            "order_by": { "key": "timestamp", "direction": "desc" }
        });

        let resp: Value = self
            .client
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
