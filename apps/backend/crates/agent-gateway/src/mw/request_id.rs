/// Request-ID middleware.
///
/// Reads `X-Request-ID` from incoming requests (or generates a UUID if absent),
/// stores it in request extensions so downstream handlers can attach it to
/// error responses, and echoes it in the `X-Request-ID` response header for
/// end-to-end correlation.
///
/// For JSON error responses (4xx/5xx) it also injects `request_id` into the
/// `{"error": {"request_id": "..."}}` body field so clients can correlate
/// errors without inspecting headers.
use axum::{body, extract::Request, http::HeaderValue, middleware::Next, response::Response};
use uuid::Uuid;

pub async fn inject_request_id(req: Request, next: Next) -> Response {
    let id = req
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(String::from)
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let mut res = next.run(req).await;

    // Always echo request-id in the response header.
    if let Ok(hv) = HeaderValue::from_str(&id) {
        res.headers_mut().insert("x-request-id", hv);
    }

    // For JSON error responses, also inject `request_id` into the `{"error": {...}}`
    // body so clients can correlate without needing to read response headers.
    let status = res.status();
    if status.is_client_error() || status.is_server_error() {
        let is_json = res
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|ct| ct.contains("application/json"))
            .unwrap_or(false);

        if is_json {
            let (parts, body_stream) = res.into_parts();
            match body::to_bytes(body_stream, 1_048_576).await {
                Ok(bytes) => {
                    let try_inject = |bytes: &[u8]| -> Option<Vec<u8>> {
                        let mut json: serde_json::Value = serde_json::from_slice(bytes).ok()?;
                        if let Some(obj) = json.get_mut("error").and_then(|e| e.as_object_mut())
                            && (!obj.contains_key("request_id") || obj["request_id"].is_null())
                        {
                            obj.insert(
                                "request_id".to_string(),
                                serde_json::Value::String(id.clone()),
                            );
                        }
                        serde_json::to_vec(&json).ok()
                    };
                    let out = try_inject(&bytes).unwrap_or_else(|| bytes.to_vec());
                    return Response::from_parts(parts, body::Body::from(out));
                }
                Err(_) => {
                    return Response::from_parts(parts, body::Body::empty());
                }
            }
        }
    }

    res
}
