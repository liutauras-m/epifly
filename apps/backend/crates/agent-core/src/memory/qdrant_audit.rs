/// QdrantAuditStore — append-only audit log backed by Qdrant.
///
/// Data layout (per tenant, collection `audit_{tenant_id}`):
///   • Each audit event is one Qdrant point with a zero-vector and full payload.
///   • Point ID is derived from the event ULID (deterministic, monotonic-friendly).
///   • Retrieval is ordered by `timestamp` descending via Qdrant's order_by API.
use super::qdrant_helpers::{VECTOR_DIM, point_id};
use async_trait::async_trait;
use common::audit::{AuditEvent, AuditStore};
use qdrant_client::Qdrant;
use qdrant_client::qdrant::{
    CreateCollectionBuilder, Direction, Distance, OrderByBuilder, PointStruct,
    ScrollPointsBuilder, UpsertPointsBuilder, VectorParamsBuilder,
};
use serde_json::json;
use std::sync::Arc;
use tracing::{instrument, warn};

pub struct QdrantAuditStore {
    client: Arc<Qdrant>,
}

impl QdrantAuditStore {
    pub fn new(grpc_url: impl Into<String>) -> Self {
        let client = Arc::new(
            Qdrant::from_url(&grpc_url.into())
                .build()
                .expect("qdrant-client build failed"),
        );
        Self { client }
    }

    fn collection(&self, tenant_id: &str) -> String {
        format!("audit_{tenant_id}")
    }

    async fn ensure_collection(&self, tenant_id: &str) -> common::error::Result<()> {
        let col = self.collection(tenant_id);
        if self.client.collection_exists(&col).await.unwrap_or(false) {
            return Ok(());
        }
        self.client
            .create_collection(
                CreateCollectionBuilder::new(&col)
                    .vectors_config(VectorParamsBuilder::new(VECTOR_DIM as u64, Distance::Cosine)),
            )
            .await
            .map_err(|e| common::error::ConusAiError::Storage(e.to_string()))?;
        Ok(())
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
        let pid = point_id(&event.id);
        let payload: qdrant_client::Payload = serde_json::to_value(&event)
            .map_err(|e| common::error::ConusAiError::Storage(e.to_string()))?
            .try_into()
            .map_err(|e| common::error::ConusAiError::Storage(format!("{e:?}")))?;

        self.client
            .upsert_points(
                UpsertPointsBuilder::new(
                    &col,
                    vec![PointStruct::new(pid, vec![0.0_f32; VECTOR_DIM], payload)],
                )
                .wait(true),
            )
            .await
            .map(|_| ())
            .map_err(|e| common::error::ConusAiError::Storage(e.to_string()))
    }

    #[instrument(skip(self), fields(tenant_id, limit))]
    async fn list(
        &self,
        tenant_id: &str,
        limit: usize,
        _after: Option<&str>,
    ) -> common::error::Result<Vec<AuditEvent>> {
        self.ensure_collection(tenant_id).await?;

        let col = self.collection(tenant_id);
        let resp = self
            .client
            .scroll(
                ScrollPointsBuilder::new(&col)
                    .order_by(OrderByBuilder::new("timestamp").direction(Direction::Desc as i32))
                    .limit(limit as u32)
                    .with_payload(true)
                    .with_vectors(false),
            )
            .await
            .map_err(|e| common::error::ConusAiError::Storage(e.to_string()))?;

        let mut events = Vec::with_capacity(resp.result.len());
        for p in resp.result {
            let payload_json = super::qdrant_helpers::payload_to_json(p.payload);
            if let Ok(ev) = serde_json::from_value::<AuditEvent>(payload_json) {
                events.push(ev);
            }
        }

        Ok(events)
    }
}

