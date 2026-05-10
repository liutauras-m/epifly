//! Unit tests for ControlMessage serialization / deserialization.
//!
//! No network or DB required — exercises pure serde logic.

use serde_json::json;

// ── Minimal local redefinitions matching shells.rs ────────────────────────────
// Integration tests cannot import private modules, so we replicate the
// structs here.  The JSON shapes must match exactly.

#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
enum ControlKind {
    Heartbeat,
    Replay,
    Stop,
    Ack,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct ControlMessage {
    kind: ControlKind,
    payload: serde_json::Value,
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[test]
fn heartbeat_serializes_to_expected_json() {
    let msg = ControlMessage {
        kind: ControlKind::Heartbeat,
        payload: serde_json::Value::Null,
    };
    let json = serde_json::to_string(&msg).expect("serialize");
    let val: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(val["kind"], "Heartbeat");
    assert!(val["payload"].is_null());
}

#[test]
fn replay_serializes_correctly() {
    let msg = ControlMessage {
        kind: ControlKind::Replay,
        payload: json!({ "trace_node_id": "abc-123", "dry_run": true }),
    };
    let json = serde_json::to_string(&msg).expect("serialize");
    let val: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(val["kind"], "Replay");
    assert_eq!(val["payload"]["trace_node_id"], "abc-123");
    assert_eq!(val["payload"]["dry_run"], true);
}

#[test]
fn stop_serializes_correctly() {
    let msg = ControlMessage {
        kind: ControlKind::Stop,
        payload: json!({ "reason": "replay_quota_exceeded" }),
    };
    let json_str = serde_json::to_string(&msg).expect("serialize");
    let val: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    assert_eq!(val["kind"], "Stop");
    assert_eq!(val["payload"]["reason"], "replay_quota_exceeded");
}

#[test]
fn heartbeat_deserializes_from_json_string() {
    let raw = r#"{"kind":"Heartbeat","payload":null}"#;
    let msg: ControlMessage = serde_json::from_str(raw).expect("deserialize");
    assert_eq!(msg.kind, ControlKind::Heartbeat);
    assert!(msg.payload.is_null());
}

#[test]
fn replay_deserializes_from_json_string() {
    let raw = r#"{"kind":"Replay","payload":{"trace_node_id":"xyz","dry_run":false}}"#;
    let msg: ControlMessage = serde_json::from_str(raw).expect("deserialize");
    assert_eq!(msg.kind, ControlKind::Replay);
    assert_eq!(msg.payload["trace_node_id"], "xyz");
    assert_eq!(msg.payload["dry_run"], false);
}

#[test]
fn ack_round_trips() {
    let msg = ControlMessage {
        kind: ControlKind::Ack,
        payload: serde_json::Value::Null,
    };
    let json_str = serde_json::to_string(&msg).expect("serialize");
    let back: ControlMessage = serde_json::from_str(&json_str).expect("deserialize");
    assert_eq!(back.kind, ControlKind::Ack);
    assert!(back.payload.is_null());
}

#[test]
fn unknown_kind_fails_to_deserialize() {
    let raw = r#"{"kind":"Unknown","payload":null}"#;
    let result: Result<ControlMessage, _> = serde_json::from_str(raw);
    assert!(
        result.is_err(),
        "unknown ControlKind must fail to deserialize"
    );
}
