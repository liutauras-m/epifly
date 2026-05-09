use super::thread::{Message, Thread, ToolCall};
use crate::types::ThreadId;
use chrono::Utc;

#[test]
fn thread_serialises_roundtrip() {
    let t = Thread {
        id: ThreadId::new(),
        tenant_id: "acme".into(),
        title: Some("Invoice discussion".into()),
        created_at: Utc::now(),
        last_active: Utc::now(),
        message_count: 3,
        summary: None,
        metadata: serde_json::json!({"source": "api"}),
    };
    let json = serde_json::to_string(&t).unwrap();
    let restored: Thread = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.id, t.id);
    assert_eq!(restored.tenant_id, t.tenant_id);
    assert_eq!(restored.message_count, 3);
}

#[test]
fn message_serialises_roundtrip() {
    let m = Message {
        role: "user".into(),
        content: "Extract this invoice".into(),
        tool_calls: None,
        timestamp: Utc::now(),
        seq: 0,
    };
    let json = serde_json::to_string(&m).unwrap();
    let restored: Message = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.role, "user");
    assert_eq!(restored.seq, 0);
}

#[test]
fn tool_call_serialises_roundtrip() {
    let tc = ToolCall {
        id: "call_abc".into(),
        name: "invoice-processing__extract_invoice".into(),
        input: serde_json::json!({"image_path": "/tmp/invoice.png"}),
        output: Some(r#"{"invoice_number": "INV-001"}"#.into()),
    };
    let json = serde_json::to_string(&tc).unwrap();
    let restored: ToolCall = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.name, tc.name);
    assert!(restored.output.is_some());
}
