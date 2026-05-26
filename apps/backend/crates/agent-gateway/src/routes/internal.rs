//! Internal routes — not exposed to the public internet.
//!
//! POST /internal/rustfs/events — bucket notification webhook (HMAC-verified).
//!
//! RustFS sends S3 event notifications to this endpoint. The gateway verifies
//! the HMAC signature, parses the event, and dispatches to the workspace indexer.

use crate::state::AppState;
use agent_core::{extract_tenant_from_legacy_key, extract_virtual_path_from_key};
use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::sync::Arc;
use tracing::{instrument, warn};

type HmacSha256 = Hmac<Sha256>;

fn verify_hmac(secret: &str, payload: &[u8], signature: &str) -> bool {
    let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.update(payload);
    // Decode the hex signature and use constant-time verify_slice to prevent timing attacks.
    let sig_hex = signature.trim_start_matches("sha256=");
    match hex::decode(sig_hex) {
        Ok(sig_bytes) => mac.verify_slice(&sig_bytes).is_ok(),
        Err(_) => false,
    }
}

/// S3 event record (subset of the full S3 notification schema).
#[derive(Debug, serde::Deserialize)]
struct S3Record {
    #[serde(rename = "eventName")]
    event_name: String,
    s3: S3Detail,
}

#[derive(Debug, serde::Deserialize)]
struct S3Detail {
    bucket: BucketDetail,
    object: ObjectDetail,
}

#[derive(Debug, serde::Deserialize)]
struct BucketDetail {
    name: String,
}

#[derive(Debug, serde::Deserialize)]
struct ObjectDetail {
    key: String,
}

#[derive(Debug, serde::Deserialize)]
struct S3EventPayload {
    #[serde(rename = "Records")]
    records: Vec<S3Record>,
}

/// POST /internal/rustfs/events — handle RustFS bucket notifications.
#[instrument(skip(state, headers, body))]
pub async fn rustfs_events(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> StatusCode {
    // HMAC verification
    let webhook_secret = match std::env::var("RUSTFS_WEBHOOK_SECRET") {
        Ok(s) => s,
        Err(_) => {
            warn!("RUSTFS_WEBHOOK_SECRET not set — rejecting all webhook events");
            return StatusCode::UNAUTHORIZED;
        }
    };

    let sig = headers
        .get("x-rustfs-signature")
        .or_else(|| headers.get("x-minio-signature"))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !verify_hmac(&webhook_secret, &body, sig) {
        warn!("rustfs event HMAC verification failed");
        return StatusCode::UNAUTHORIZED;
    }

    let payload: S3EventPayload = match serde_json::from_slice(&body) {
        Ok(p) => p,
        Err(e) => {
            warn!(error = %e, "failed to parse S3 event payload");
            return StatusCode::BAD_REQUEST;
        }
    };

    for record in &payload.records {
        let key = &record.s3.object.key;
        let event = &record.event_name;
        let bucket = &record.s3.bucket.name;

        // Extract tenant_id — try legacy key prefix first, then modern bucket name.
        let tenant_id =
            extract_tenant_from_legacy_key(key).or_else(|| extract_tenant_from_bucket(bucket));
        let Some(tenant_id) = tenant_id else {
            warn!(
                key,
                bucket, "could not extract tenant_id from object key or bucket"
            );
            continue;
        };

        tracing::info!(event, key, tenant_id, "RustFS event received");

        if event.starts_with("s3:ObjectCreated") || event.starts_with("s3:ObjectRemoved") {
            let is_delete = event.starts_with("s3:ObjectRemoved");
            let virtual_path = extract_virtual_path_from_key(key);
            let vector_store = Arc::clone(&state.vector_store);
            let embedding_svc = Arc::clone(&state.embedding_service);
            let workspace_content = Arc::clone(&state.workspace_content);
            let tenant_id_owned = tenant_id.to_string();
            let virtual_path_owned = virtual_path.to_string();

            tokio::spawn(async move {
                if is_delete {
                    // Remove vectors for this document
                    if let Err(e) = vector_store
                        .delete_content_embeddings_for_doc(&virtual_path_owned)
                        .await
                    {
                        warn!(error = %e, virtual_path = %virtual_path_owned, "failed to remove vectors");
                    }
                } else {
                    // Fetch, chunk, embed, upsert
                    let content = match workspace_content
                        .read(&tenant_id_owned, &virtual_path_owned)
                        .await
                    {
                        Ok(c) => c,
                        Err(e) => {
                            warn!(error = %e, "event indexing: failed to read content");
                            return;
                        }
                    };

                    if content.is_empty() {
                        return;
                    }

                    const CHUNK: usize = 1500;
                    let chunks: Vec<String> = content
                        .chars()
                        .collect::<Vec<_>>()
                        .chunks(CHUNK)
                        .map(|c| c.iter().collect::<String>())
                        .collect();

                    if let Ok(embeddings) = embedding_svc.embed_documents(chunks.clone()).await {
                        for (i, (chunk, emb)) in chunks.iter().zip(embeddings.iter()).enumerate() {
                            let chunk_id = format!("{virtual_path_owned}_{i}");
                            let _ = vector_store
                                .upsert_content_embedding_full(
                                    &chunk_id,
                                    &virtual_path_owned,
                                    i as i32,
                                    chunk,
                                    emb,
                                    &tenant_id_owned,
                                    &tenant_id_owned,
                                    &[],
                                )
                                .await;
                        }
                    }
                }
            });
        }
    }

    StatusCode::NO_CONTENT
}

fn extract_tenant_from_bucket(bucket_name: &str) -> Option<&str> {
    // Modern bucket name: ws-{tenant_id}
    bucket_name.strip_prefix("ws-")
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State;
    use axum::http::HeaderValue;
    use hmac::Mac;
    use std::sync::{Arc, Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn verify_hmac_accepts_valid_signature() {
        let payload = br#"{"Records":[]}"#;
        let secret = "test-secret";

        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("hmac");
        mac.update(payload);
        let sig = mac.finalize().into_bytes();
        let header = format!("sha256={}", hex::encode(sig));

        assert!(verify_hmac(secret, payload, &header));
    }

    #[test]
    fn verify_hmac_rejects_invalid_signature() {
        let payload = br#"{"Records":[]}"#;
        let secret = "test-secret";
        assert!(!verify_hmac(secret, payload, "sha256=deadbeef"));
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn rustfs_events_rejects_missing_secret() {
        let _guard = env_lock().lock().expect("env lock");
        // Safety: test-only env mutation guarded by a process-local mutex.
        unsafe {
            std::env::remove_var("RUSTFS_WEBHOOK_SECRET");
        }

        let state = Arc::new(crate::state::AppState::with_in_memory_stores().expect("state"));
        let status = rustfs_events(
            State(state),
            HeaderMap::new(),
            Bytes::from_static(br#"{"Records":[]}"#),
        )
        .await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn rustfs_events_rejects_bad_signature() {
        let _guard = env_lock().lock().expect("env lock");
        // Safety: test-only env mutation guarded by a process-local mutex.
        unsafe {
            std::env::set_var("RUSTFS_WEBHOOK_SECRET", "test-secret");
        }

        let state = Arc::new(crate::state::AppState::with_in_memory_stores().expect("state"));
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-rustfs-signature",
            HeaderValue::from_static("sha256=deadbeef"),
        );

        let status = rustfs_events(
            State(state),
            headers,
            Bytes::from_static(br#"{"Records":[]}"#),
        )
        .await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
}
