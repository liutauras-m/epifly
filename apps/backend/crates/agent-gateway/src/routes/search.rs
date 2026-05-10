/// Semantic capability search backed by Postgres pgvector ANN retrieval.
///
/// GET /v1/capabilities/search?q=finance&limit=5
///
/// On each request, capability cards are upserted into `capability_embeddings`
/// (with embedding) when their content has changed (hash-based check).
/// The query is embedded and top-N results are retrieved via cosine ANN search.
/// Falls back to local substring matching only when the vector path fails.
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
use tracing::{instrument, warn};

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
#[instrument(skip(state, tenant, query))]
pub async fn search(
    State(state): State<Arc<AppState>>,
    Extension(tenant): Extension<ResolvedTenant>,
    Query(query): Query<SearchQuery>,
) -> Json<Value> {
    let limit = query.limit.unwrap_or(5).min(20) as usize;

    let cards: Vec<_> = {
        let reg = state.registry.lock().unwrap();
        reg.enabled_for_tenant(&tenant.0.tenant_id).cloned().collect()
    };

    match vector_search(&state, &query.q, &cards, limit).await {
        Ok(results) => Json(json!({
            "tenant_id": tenant.0.tenant_id,
            "query": query.q,
            "results": results,
            "source": "vector"
        })),
        Err(e) => {
            warn!(error = %e, "vector capability search failed; falling back to local");
            Json(local_search(&tenant.0.tenant_id, &query.q, &cards, limit))
        }
    }
}

// ── Vector search path ────────────────────────────────────────────────────────

async fn vector_search(
    state: &AppState,
    query: &str,
    cards: &[agent_core::capabilities::card::CapabilityCard],
    limit: usize,
) -> anyhow::Result<Vec<Value>> {
    // 1. Refresh capability embeddings for changed cards.
    refresh_capability_embeddings(state, cards).await?;

    // 2. Embed the query.
    let query_embedding = state.embedding_service.embed_query(query).await?;

    // 3. ANN retrieval.
    let hits = state
        .vector_store
        .top_n_capabilities(&query_embedding, limit)
        .await?;

    Ok(hits
        .into_iter()
        .map(|h| {
            json!({
                "name":     h.capability_id,
                "content":  h.content,
                "metadata": h.metadata,
                "score":    1.0 - h.distance,
            })
        })
        .collect())
}

/// Upsert capability cards into `capability_embeddings` when their content has
/// changed.  Uses a SHA-256 hash stored in `metadata.content_hash` to skip
/// unchanged cards.
async fn refresh_capability_embeddings(
    state: &AppState,
    cards: &[agent_core::capabilities::card::CapabilityCard],
) -> anyhow::Result<()> {
    for card in cards {
        let content = format!(
            "{} {} {} {}",
            card.manifest.name,
            card.manifest.description,
            card.namespace(),
            card.manifest.tags.join(" ")
        );

        let content_hash = {
            let mut h = Sha256::new();
            h.update(content.as_bytes());
            format!("{:x}", h.finalize())
        };

        // Fetch stored hash.
        let stored_hash: Option<String> = if let Some(pool) = &state.pool {
            sqlx::query_scalar!(
                "SELECT (metadata->>'content_hash')::text
             FROM capability_embeddings WHERE capability_id = $1",
                card.manifest.name,
            )
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
            .flatten()
        } else {
            None
        };

        if stored_hash.as_deref() == Some(&content_hash) {
            continue; // unchanged — skip re-embedding
        }

        // Generate embedding.
        let embedding = match state.embedding_service.embed_query(&content).await {
            Ok(v) => v,
            Err(e) => {
                warn!(capability = %card.manifest.name, error = %e, "embedding generation failed; skipping card");
                continue;
            }
        };

        let metadata = json!({
            "kind":         format!("{:?}", card.manifest.kind),
            "tags":         card.manifest.tags,
            "namespace":    card.namespace(),
            "content_hash": content_hash,
        });

        if let Err(e) = state
            .vector_store
            .upsert_capability_embedding_full(
                &card.manifest.name,
                &content,
                &embedding,
                &metadata,
                card.namespace(),
                card.tags(),
            )
            .await
        {
            warn!(capability = %card.manifest.name, error = %e, "capability embedding upsert failed");
        }
    }
    Ok(())
}

// ── Local fallback ────────────────────────────────────────────────────────────

fn local_search(
    tenant_id: &str,
    query: &str,
    cards: &[agent_core::capabilities::card::CapabilityCard],
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
