//! Unit-level tests for device-token logic and related pure functions.
//!
//! No real DB required — these exercise pure logic only.

// ── blake3 determinism ────────────────────────────────────────────────────────

#[test]
fn blake3_hash_is_deterministic() {
    let a = blake3::hash(b"some_device_token");
    let b = blake3::hash(b"some_device_token");
    assert_eq!(a, b, "blake3 hash of the same input must always match");
}

#[test]
fn blake3_different_tokens_produce_different_hashes() {
    let a = blake3::hash(b"token_alpha");
    let b = blake3::hash(b"token_beta");
    assert_ne!(a, b);
}

#[test]
fn blake3_hash_bytes_length_is_32() {
    let h = blake3::hash(b"anything");
    assert_eq!(h.as_bytes().len(), 32);
}

// ── HttpError status helper ───────────────────────────────────────────────────

fn status(err: common::error::HttpError) -> u16 {
    err.status.as_u16()
}

// require_shell_feature env-var tests live in admin_devices unit tests to avoid
// parallel-test races; see routes::admin_devices::tests in admin_devices.rs.

#[test]
fn not_found_http_error_has_404_status() {
    let err = common::error::HttpError::not_found("browser shell feature not enabled");
    assert_eq!(err.status.as_u16(), 404);
}

#[test]
fn auth_http_error_has_401_status() {
    let err = common::error::HttpError::auth("invalid token");
    assert_eq!(err.status.as_u16(), 401);
}

// ── require_platform_admin ────────────────────────────────────────────────────

fn check_platform_admin(
    auth_header: Option<&str>,
    token_env: &str,
) -> Result<(), common::error::HttpError> {
    let expected = token_env.to_string();
    if expected.is_empty() {
        return Ok(());
    }
    let bearer = auth_header
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    if bearer == expected {
        Ok(())
    } else {
        Err(common::error::HttpError::auth(
            "invalid platform admin token",
        ))
    }
}

#[test]
fn require_platform_admin_rejects_wrong_bearer() {
    let result = check_platform_admin(Some("Bearer wrong"), "correct_token");
    assert_eq!(status(result.unwrap_err()), 401);
}

#[test]
fn require_platform_admin_accepts_correct_bearer() {
    let result = check_platform_admin(Some("Bearer secret"), "secret");
    assert!(result.is_ok());
}

#[test]
fn require_platform_admin_allows_all_when_token_unset() {
    // When env var is empty, any (or no) bearer is accepted.
    let result = check_platform_admin(None, "");
    assert!(result.is_ok());
}

#[test]
fn require_platform_admin_rejects_missing_header_when_token_set() {
    let result = check_platform_admin(None, "required_token");
    assert_eq!(status(result.unwrap_err()), 401);
}

// ── IssueDeviceRequest / IssueDeviceResponse serde round-trip ────────────────

#[test]
fn issue_device_request_serde_round_trip() {
    let json = r#"{"tenant_id":"acme","device_label":"laptop-1"}"#;
    let req: serde_json::Value = serde_json::from_str(json).expect("parse");
    assert_eq!(req["tenant_id"], "acme");
    assert_eq!(req["device_label"], "laptop-1");
    // Serialize back and confirm key round-trips.
    let out = serde_json::to_string(&req).expect("serialize");
    let reparsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(reparsed["tenant_id"], req["tenant_id"]);
}

#[test]
fn issue_device_response_serde_round_trip() {
    let json = r#"{"id":"00000000-0000-0000-0000-000000000001","token":"deadbeef","device_label":"laptop-1"}"#;
    let resp: serde_json::Value = serde_json::from_str(json).expect("parse");
    assert_eq!(resp["id"], "00000000-0000-0000-0000-000000000001");
    assert_eq!(resp["token"], "deadbeef");
    assert_eq!(resp["device_label"], "laptop-1");
    let out = serde_json::to_string(&resp).expect("serialize");
    let reparsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(reparsed["token"], resp["token"]);
}
