/// Semantic capability search backed by Qdrant.
///
/// GET /v1/capabilities/search?q=finance&limit=5
///
/// On first call per tenant the collection is created and all capability
/// embeddings are upserted.  Subsequent calls hit the Qdrant search API.
/// Falls back to local substring matching when Qdrant is unreachable.
use crate::mw::tenant::ResolvedTenant;
use crate::state::AppState;
use axum::{
    Extension, Json,
    extract::{Query, State},
};
use qdrant_client::Qdrant;
use qdrant_client::qdrant::{
    CreateCollectionBuilder, Distance, PointStruct, SearchPointsBuilder, UpsertPointsBuilder,
    VectorParamsBuilder,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tracing::{info, warn};

const VECTOR_DIMS: usize = 64;

#[derive(Deserialize, utoipa::IntoParams)]
pub struct SearchQuery {
    pub q: String,
    pub limit: Option<u32>,
}

#[utoipa::path(
    get,
    path = "/v1/capabilities/search",
    params(SearchQuery),
    responses(
        (status = 200, description = "Matching capabilities", body = Value),
    ),
    security(("bearer_auth" = [])),
    tag = "capabilities",
)]
pub async fn search(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    Query(query): Query<SearchQuery>,
) -> Json<Value> {
    let limit = query.limit.unwrap_or(5).min(20) as usize;
    let grpc_url =
        std::env::var("QDRANT_GRPC_URL").unwrap_or_else(|_| "http://localhost:6334".into());
    let collection = tenant.0.qdrant_collection("capabilities");

    // Collect cards once (outside lock)
    let cards: Vec<_> = {
        let reg = state.registry.lock().unwrap();
        reg.all_enabled().cloned().collect()
    };

    let client = match Qdrant::from_url(&grpc_url).build() {
        Ok(c) => Arc::new(c),
        Err(e) => {
            warn!(error = %e, "Qdrant client build failed; falling back to local search");
            return Json(local_search(&tenant.0.tenant_id, &query.q, &cards, limit));
        }
    };

    // Ensure the Qdrant collection is seeded for this tenant
    if let Err(e) = ensure_collection(&client, &collection, &cards).await {
        warn!(error = %e, collection, "Qdrant seed failed; falling back to local search");
        return Json(local_search(&tenant.0.tenant_id, &query.q, &cards, limit));
    }

    // Vector search
    let q_vec = text_to_vec(&query.q);
    match qdrant_search(&client, &collection, q_vec, limit as u64).await {
        Ok(results) => Json(json!({
            "tenant_id": tenant.0.tenant_id,
            "query": query.q,
            "results": results,
            "source": "qdrant"
        })),
        Err(e) => {
            warn!(error = %e, "Qdrant search failed; falling back to local");
            Json(local_search(&tenant.0.tenant_id, &query.q, &cards, limit))
        }
    }
}

// ── Qdrant helpers ─────────────────────────────────────────────────────────────

/// Create the collection if absent and upsert all capability vectors.
async fn ensure_collection(
    client: &Qdrant,
    collection: &str,
    cards: &[agent_core::tools::card::CapabilityCard],
) -> anyhow::Result<()> {
    if !client.collection_exists(collection).await? {
        client
            .create_collection(
                CreateCollectionBuilder::new(collection).vectors_config(
                    VectorParamsBuilder::new(VECTOR_DIMS as u64, Distance::Cosine),
                ),
            )
            .await?;
        info!(collection, "Qdrant collection created");
    }

    // Upsert all capability embeddings
    if !cards.is_empty() {
        let points: Vec<PointStruct> = cards
            .iter()
            .enumerate()
            .map(|(i, card)| {
                let embedding_text = card.manifest.embedding_text();
                let payload: qdrant_client::Payload = json!({
                    "name":        card.manifest.name,
                    "description": card.manifest.description,
                    "kind":        format!("{:?}", card.manifest.kind),
                    "tags":        card.manifest.tags,
                })
                .try_into()
                .expect("payload conversion");
                PointStruct::new((i + 1) as u64, text_to_vec(&embedding_text), payload)
            })
            .collect();

        client
            .upsert_points(UpsertPointsBuilder::new(collection, points))
            .await?;
    }

    Ok(())
}

async fn qdrant_search(
    client: &Qdrant,
    collection: &str,
    vector: Vec<f32>,
    limit: u64,
) -> anyhow::Result<Vec<Value>> {
    let resp = client
        .search_points(
            SearchPointsBuilder::new(collection, vector, limit).with_payload(true),
        )
        .await?;

    let hits = resp
        .result
        .iter()
        .map(|r| {
            let p = agent_core::memory::qdrant_helpers::payload_to_json(r.payload.clone());
            json!({
                "name":        p["name"],
                "description": p["description"],
                "kind":        p["kind"],
                "tags":        p["tags"],
                "score":       r.score
            })
        })
        .collect();

    Ok(hits)
}

// ── Fallback: local substring search ─────────────────────────────────────────

fn local_search(
    tenant_id: &str,
    query: &str,
    cards: &[agent_core::tools::card::CapabilityCard],
    limit: usize,
) -> Value {
    let q = query.to_lowercase();
    let results: Vec<Value> = cards
        .iter()
        .filter(|c| {
            c.manifest.name.to_lowercase().contains(&q)
                || c.manifest.description.to_lowercase().contains(&q)
                || c.manifest
                    .tags
                    .iter()
                    .any(|t| t.to_lowercase().contains(&q))
        })
        .take(limit)
        .map(|c| {
            json!({
                "name":        c.manifest.name,
                "description": c.manifest.description,
                "kind":        format!("{:?}", c.manifest.kind),
                "tags":        c.manifest.tags,
                "score":       1.0
            })
        })
        .collect();

    json!({
        "tenant_id": tenant_id,
        "query": query,
        "results": results,
        "source": "local"
    })
}

// ── Embedding helper ──────────────────────────────────────────────────────────

/// Deterministic hash-based vector.  Not semantically meaningful but exercises
/// the Qdrant data plane with real writes and nearest-neighbour queries.
/// Replace with a real embedding model for production semantic search.
pub fn text_to_vec(text: &str) -> Vec<f32> {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    let hash = hasher.finalize();
    let hash_bytes = hash.as_slice(); // 32 bytes

    (0..VECTOR_DIMS)
        .map(|i| {
            let b = hash_bytes[i % hash_bytes.len()];
            // Rotate to decorrelate dimensions
            let rotated = b.rotate_left((i % 8) as u32);
            (rotated as f32 / 127.5) - 1.0 // [-1, 1]
        })
        .collect()
}


