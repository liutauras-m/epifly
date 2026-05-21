//! Namespace filter for capability routing and vector-store queries.
//!
//! `NamespaceFilter` is the canonical way to restrict capability look-ups by
//! namespace.  It is evaluated in-memory (`matches`) for Qdrant filter building.

/// Filter capabilities by their primary `namespace` field.
#[derive(Debug, Clone, Default)]
pub enum NamespaceFilter {
    /// No restriction — matches everything (the default).
    #[default]
    Any,
    /// Matches only the exact namespace string.
    Exact(String),
    /// Matches namespaces that start with `prefix` (e.g. `"accounting."` matches
    /// `"accounting.gl"` and `"accounting.ap"` but not `"accounting"` itself unless
    /// the prefix ends without the dot separator).
    Prefix(String),
    /// Union of multiple filters — matches if any sub-filter matches.
    AnyOf(Vec<NamespaceFilter>),
}

impl NamespaceFilter {
    /// Returns `true` when `ns` passes this filter.
    pub fn matches(&self, ns: &str) -> bool {
        match self {
            NamespaceFilter::Any => true,
            NamespaceFilter::Exact(e) => ns == e.as_str(),
            NamespaceFilter::Prefix(p) => ns.starts_with(p.as_str()),
            NamespaceFilter::AnyOf(filters) => filters.iter().any(|f| f.matches(ns)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_matches() {
        let f = NamespaceFilter::Exact("accounting.gl".into());
        assert!(f.matches("accounting.gl"));
        assert!(!f.matches("accounting.ap"));
        assert!(!f.matches("accounting"));
    }

    #[test]
    fn prefix_matches() {
        let f = NamespaceFilter::Prefix("accounting.".into());
        assert!(f.matches("accounting.gl"));
        assert!(f.matches("accounting.ap"));
        assert!(!f.matches("accounting"));
        assert!(!f.matches("payroll.run"));
    }

    #[test]
    fn any_of_matches() {
        let f = NamespaceFilter::AnyOf(vec![
            NamespaceFilter::Exact("erp.po".into()),
            NamespaceFilter::Prefix("accounting.".into()),
        ]);
        assert!(f.matches("erp.po"));
        assert!(f.matches("accounting.gl"));
        assert!(!f.matches("payroll.run"));
    }
}
