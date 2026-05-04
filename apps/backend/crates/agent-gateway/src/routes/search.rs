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
    let qdrant_url = std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6333".into());
    let collection = tenant.0.qdrant_collection("capabilities");

    // Collect cards once (outside lock)
    let cards: Vec<_> = {
        let reg = state.registry.lock().unwrap();
        reg.all_enabled().cloned().collect()
    };

    // Ensure the Qdrant collection is seeded for this tenant
    if let Err(e) = ensure_collection(&qdrant_url, &collection, &cards).await {
        warn!(error = %e, collection, "Qdrant seed failed; falling back to local search");
        return Json(local_search(&tenant.0.tenant_id, &query.q, &cards, limit));
    }

    // Vector search
    let q_vec = text_to_vec(&query.q);
    match qdrant_search(&qdrant_url, &collection, q_vec, limit as u64).await {
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
    base: &str,
    collection: &str,
    cards: &[agent_core::tools::card::ToolCard],
) -> anyhow::Result<()> {
    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;

    // Check existence
    let check = http
        .get(format!("{base}/collections/{collection}"))
        .send()
        .await?;

    if check.status() == 404 {
        // Create collection
        http.put(format!("{base}/collections/{collection}"))
            .json(&json!({
                "vectors": { "size": VECTOR_DIMS, "distance": "Cosine" }
            }))
            .send()
            .await?;

        info!(collection, "Qdrant collection created");
    }

    // Upsert all capability embeddings
    let points: Vec<Value> = cards
        .iter()
        .enumerate()
        .map(|(i, card)| {
            let embedding_text = card.manifest.embedding_text();
            json!({
                "id": i + 1,
                "vector": text_to_vec(&embedding_text),
                "payload": {
                    "name":        card.manifest.name,
                    "description": card.manifest.description,
                    "kind":        format!("{:?}", card.manifest.kind),
                    "tags":        card.manifest.tags,
                }
            })
        })
        .collect();

    if !points.is_empty() {
        http.put(format!("{base}/collections/{collection}/points"))
            .json(&json!({ "points": points }))
            .send()
            .await?;
    }

    Ok(())
}

async fn qdrant_search(
    base: &str,
    collection: &str,
    vector: Vec<f32>,
    limit: u64,
) -> anyhow::Result<Vec<Value>> {
    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;

    let resp = http
        .post(format!("{base}/collections/{collection}/points/search"))
        .json(&json!({
            "vector": vector,
            "limit": limit,
            "with_payload": true
        }))
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Qdrant search returned {status}: {body}");
    }

    let body: Value = resp.json().await?;
    let hits = body["result"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .map(|r| {
            json!({
                "name":        r["payload"]["name"],
                "description": r["payload"]["description"],
                "kind":        r["payload"]["kind"],
                "tags":        r["payload"]["tags"],
                "score":       r["score"]
            })
        })
        .collect();

    Ok(hits)
}

// ── Fallback: local substring search ─────────────────────────────────────────

fn local_search(
    tenant_id: &str,
    query: &str,
    cards: &[agent_core::tools::card::ToolCard],
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
