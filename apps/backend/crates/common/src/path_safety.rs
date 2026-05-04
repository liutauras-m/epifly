use std::path::{Component, Path, PathBuf};

/// Safe join that rejects path traversal attempts.
pub fn safe_join(base: &Path, untrusted: &str) -> crate::error::Result<PathBuf> {
    let candidate = base.join(untrusted);
    let canonical = candidate.components().fold(PathBuf::new(), |mut acc, c| {
        match c {
            Component::ParentDir => {
                acc.pop();
            }
            Component::Normal(p) => acc.push(p),
            Component::RootDir => acc.push("/"),
            _ => {}
        }
        acc
    });
    if !canonical.starts_with(base) {
        return Err(crate::error::ConusAiError::Tool(format!(
            "path traversal attempt: {untrusted}"
        )));
    }
    Ok(canonical)
}

/// Join a relative path under `{root}/tenants/{tenant_id}/`.
/// Rejects traversal attempts out of the tenant sandbox.
pub fn join_under_tenant(root: &Path, tenant_id: &str, rel: &str) -> crate::error::Result<PathBuf> {
    // tenant_id itself must not contain traversal characters
    if tenant_id.contains("..") || tenant_id.contains('/') || tenant_id.contains('\\') {
        return Err(crate::error::ConusAiError::Tool(format!(
            "invalid tenant_id: {tenant_id}"
        )));
    }
    let tenant_root = root.join("tenants").join(tenant_id);
    safe_join(&tenant_root, rel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tenant_join_valid() {
        let root = Path::new("/workspaces");
        let p = join_under_tenant(root, "acme", "invoices/q1.pdf").unwrap();
        assert_eq!(p, PathBuf::from("/workspaces/tenants/acme/invoices/q1.pdf"));
    }

    #[test]
    fn tenant_join_rejects_traversal_in_rel() {
        let root = Path::new("/workspaces");
        assert!(join_under_tenant(root, "acme", "../../etc/passwd").is_err());
    }

    #[test]
    fn tenant_join_rejects_bad_tenant_id() {
        let root = Path::new("/workspaces");
        assert!(join_under_tenant(root, "../evil", "file.txt").is_err());
    }
}
