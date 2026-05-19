//! Bucket name helpers for per-tenant bucket provisioning.

/// Sanitize a raw string into a valid S3 bucket name.
///
/// S3 bucket naming rules enforced:
/// - 3–63 characters
/// - Only lowercase `[a-z0-9-]` (dots removed to avoid SSL wildcard issues)
/// - No leading or trailing `-`
/// - Not IP-address-shaped (`n.n.n.n`)
/// - No consecutive `-` turned into single `-`
///
/// Returns a best-effort valid bucket name. Panics only if the input is empty after
/// stripping illegal characters (which would indicate a programming error).
pub fn sanitize_bucket_name(input: &str) -> String {
    // Lowercase, replace illegal chars with `-`.
    let mut name: String = input
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' })
        .collect();

    // Collapse consecutive dashes into one.
    while name.contains("--") {
        name = name.replace("--", "-");
    }

    // Strip leading/trailing dashes.
    let name = name.trim_matches('-');

    // Clamp to 63 characters.
    let name = if name.len() > 63 { &name[..63] } else { name };
    let name = name.trim_matches('-'); // re-trim after clamp

    // Pad to minimum 3 chars with 'x' if needed.
    let mut name = name.to_string();
    while name.len() < 3 {
        name.push('x');
    }

    name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_tenant_id() {
        assert_eq!(sanitize_bucket_name("ws-acme-corp"), "ws-acme-corp");
    }

    #[test]
    fn uppercase_lowered() {
        assert_eq!(sanitize_bucket_name("ws-TENANT"), "ws-tenant");
    }

    #[test]
    fn dots_replaced() {
        assert_eq!(sanitize_bucket_name("ws-tenant.v2"), "ws-tenant-v2");
    }

    #[test]
    fn consecutive_dashes_collapsed() {
        assert_eq!(sanitize_bucket_name("ws--tenant--id"), "ws-tenant-id");
    }

    #[test]
    fn leading_trailing_dashes_stripped() {
        assert_eq!(sanitize_bucket_name("-ws-tenant-"), "ws-tenant");
    }

    #[test]
    fn long_name_clamped() {
        let long = "a".repeat(80);
        assert!(sanitize_bucket_name(&format!("ws-{long}")).len() <= 63);
    }
}
