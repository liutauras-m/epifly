//! Helpers for handling shared secrets and bearer credentials.

use sha2::{Digest, Sha256};

/// Constant-time equality for secrets.
///
/// Both inputs are hashed first so the comparison time depends on neither the
/// position of the first differing byte nor the length of the secret.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let da = Sha256::digest(a);
    let db = Sha256::digest(b);
    da.iter()
        .zip(db.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

/// The credential of an `Authorization: Bearer <token>` header, if present and non-empty.
pub fn bearer_token(headers: &axum::http::HeaderMap) -> Option<&str> {
    headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|t| !t.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_and_unequal() {
        assert!(constant_time_eq(b"secret", b"secret"));
        assert!(!constant_time_eq(b"secret", b"secreT"));
        assert!(!constant_time_eq(b"secret", b"secret-longer"));
        assert!(!constant_time_eq(b"", b"x"));
    }

    #[test]
    fn bearer_parsing() {
        let mut h = axum::http::HeaderMap::new();
        assert_eq!(bearer_token(&h), None);
        h.insert("authorization", "Bearer abc".parse().unwrap());
        assert_eq!(bearer_token(&h), Some("abc"));
        h.insert("authorization", "Bearer ".parse().unwrap());
        assert_eq!(bearer_token(&h), None);
        h.insert("authorization", "Basic abc".parse().unwrap());
        assert_eq!(bearer_token(&h), None);
    }
}
