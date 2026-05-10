/// Minimal fire-and-forget OTel span emitter over OTLP/HTTP JSON.
///
/// Reads `OTLP_ENDPOINT` env var (default `http://localhost:4318`).
/// Errors are silently ignored — telemetry is non-critical.
pub async fn emit_span(name: &str, attrs: &[(&str, &str)]) {
    let endpoint =
        std::env::var("OTLP_ENDPOINT").unwrap_or_else(|_| "http://localhost:4318".to_owned());

    let platform = if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    };

    let now_ns = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);

    // Build trace/span IDs from two ULIDs (truncated to the required byte lengths).
    let trace_id = {
        let u1 = ulid::Ulid::new().to_bytes();
        let u2 = ulid::Ulid::new().to_bytes();
        // 16 bytes → 32 hex chars
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&[u1, u2].concat()[..16]);
        hex_encode(&bytes)
    };
    let span_id = {
        let u = ulid::Ulid::new().to_bytes();
        // 8 bytes → 16 hex chars
        hex_encode(&u[..8])
    };

    // Merge built-in attributes with caller-supplied ones.
    let mut attribute_json: Vec<serde_json::Value> = vec![
        attr_kv("platform", platform),
        attr_kv("shell.kind", "browser"),
    ];
    for (k, v) in attrs {
        attribute_json.push(attr_kv(k, v));
    }

    let payload = serde_json::json!({
        "resourceSpans": [{
            "resource": {
                "attributes": [attr_kv("service.name", "browser-shell")]
            },
            "scopeSpans": [{
                "spans": [{
                    "traceId": trace_id,
                    "spanId": span_id,
                    "name": name,
                    "startTimeUnixNano": now_ns.to_string(),
                    "endTimeUnixNano": now_ns.to_string(),
                    "attributes": attribute_json
                }]
            }]
        }]
    });

    let url = format!("{endpoint}/v1/traces");
    let client = reqwest::Client::new();
    // Intentionally ignore all errors.
    let _ = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await;
}

fn attr_kv(key: &str, value: &str) -> serde_json::Value {
    serde_json::json!({
        "key": key,
        "value": { "stringValue": value }
    })
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
