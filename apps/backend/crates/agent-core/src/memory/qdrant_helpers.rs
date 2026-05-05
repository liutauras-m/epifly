//! Minimal Qdrant utilities shared by all three stores.
//!
//! The old REST-HTTP wrapper (`QdrantClient`) was removed in v0.3; all stores now use
//! `qdrant_client::Qdrant` (gRPC) directly.  This file contains only the two deterministic
//! helpers that every store needs.
use qdrant_client::qdrant::{Value as QValue, value::Kind, ListValue, Struct};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

pub const VECTOR_DIM: usize = 4;

/// Derive a stable u64 Qdrant point ID from any string key (first 8 bytes of SHA-256).
pub fn point_id(key: &str) -> u64 {
    let mut h = Sha256::new();
    h.update(key.as_bytes());
    let digest = h.finalize();
    u64::from_le_bytes(digest[..8].try_into().unwrap())
}

/// Convert a `RetrievedPoint.payload` map (protobuf `Value`) to a `serde_json::Value` object.
///
/// Used by all three stores to reconstruct stored documents after a scroll or get.
pub fn payload_to_json(payload: HashMap<String, QValue>) -> Value {
    let map: Map<String, Value> = payload
        .into_iter()
        .map(|(k, v)| (k, qdrant_value_to_json(v)))
        .collect();
    Value::Object(map)
}

fn qdrant_value_to_json(v: QValue) -> Value {
    match v.kind {
        None | Some(Kind::NullValue(_)) => Value::Null,
        Some(Kind::BoolValue(b)) => Value::Bool(b),
        Some(Kind::IntegerValue(i)) => Value::Number(i.into()),
        Some(Kind::DoubleValue(f)) => {
            serde_json::Number::from_f64(f)
                .map(Value::Number)
                .unwrap_or(Value::Null)
        }
        Some(Kind::StringValue(s)) => Value::String(s),
        Some(Kind::ListValue(ListValue { values, .. })) => {
            Value::Array(values.into_iter().map(qdrant_value_to_json).collect())
        }
        Some(Kind::StructValue(Struct { fields })) => {
            let map: Map<String, Value> = fields
                .into_iter()
                .map(|(k, v)| (k, qdrant_value_to_json(v)))
                .collect();
            Value::Object(map)
        }
    }
}

