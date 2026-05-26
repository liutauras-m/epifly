//! Minimal AWS SigV4 signing for RustFS admin API requests.

use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

type HmacSha256 = Hmac<Sha256>;

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC key length valid");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn signing_key(secret_key: &str, date: &str, region: &str, service: &str) -> Vec<u8> {
    let k_secret = format!("AWS4{secret_key}");
    let k_date = hmac_sha256(k_secret.as_bytes(), date.as_bytes());
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, service.as_bytes());
    hmac_sha256(&k_service, b"aws4_request")
}

/// Attach an `Authorization` header (SigV4) to a mutable header map.
///
/// `request.method` must be uppercase (e.g. "PUT").
/// `request.uri_path` is the percent-encoded path (e.g. "/minio/admin/v3/add-user").
/// `request.query` is already-encoded query string (e.g. "accessKey=tenant-foo").
/// `request.extra_headers` is additional headers to sign (beyond host and x-amz-date).
/// `request.payload` is the raw body bytes (use b"" for no body).
pub struct SigningContext<'a> {
    pub host: &'a str,
    pub access_key: &'a str,
    pub secret_key: &'a str,
    pub region: &'a str,
    pub service: &'a str,
}

pub struct SigningRequest<'a> {
    pub method: &'a str,
    pub uri_path: &'a str,
    pub query: &'a str,
    pub extra_headers: &'a BTreeMap<String, String>,
    pub payload: &'a [u8],
}

pub fn sign_request(
    ctx: &SigningContext<'_>,
    request: &SigningRequest<'_>,
) -> BTreeMap<String, String> {
    let now = Utc::now();
    let datetime = now.format("%Y%m%dT%H%M%SZ").to_string();
    let date = now.format("%Y%m%d").to_string();

    let payload_hash = sha256_hex(request.payload);

    let mut headers: BTreeMap<String, String> = BTreeMap::new();
    headers.insert("host".into(), ctx.host.to_string());
    headers.insert("x-amz-date".into(), datetime.clone());
    headers.insert("x-amz-content-sha256".into(), payload_hash.clone());
    for (k, v) in request.extra_headers {
        headers.insert(k.to_lowercase(), v.clone());
    }

    let canonical_headers: String = headers
        .iter()
        .map(|(k, v)| format!("{}:{}\n", k, v.trim()))
        .collect();
    let signed_headers: String = headers.keys().cloned().collect::<Vec<_>>().join(";");

    let canonical_request = format!(
        "{}\n{}\n{}\n{canonical_headers}\n{signed_headers}\n{payload_hash}",
        request.method, request.uri_path, request.query
    );

    let scope = format!("{date}/{}/{}/aws4_request", ctx.region, ctx.service);
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{datetime}\n{scope}\n{}",
        sha256_hex(canonical_request.as_bytes())
    );

    let key = signing_key(ctx.secret_key, &date, ctx.region, ctx.service);
    let sig = hex::encode(hmac_sha256(&key, string_to_sign.as_bytes()));

    let auth = format!(
        "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed_headers}, Signature={sig}",
        ctx.access_key
    );

    let mut result = headers;
    result.insert("authorization".into(), auth);
    result
}
