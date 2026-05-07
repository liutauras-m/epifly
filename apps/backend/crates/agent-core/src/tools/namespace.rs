//! Namespace filter for capability routing and vector-store queries.
//!
//! `NamespaceFilter` is the canonical way to restrict capability look-ups by
//! namespace.  It can be evaluated in-memory (`matches`) and compiled to a
//! Postgres predicate (`to_sql_predicate`) for use in `PgVectorStore` queries.

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

    /// Build a Postgres SQL fragment suitable for inclusion in a WHERE clause.
    ///
    /// `col` is the column name (e.g. `"namespace"`).
    /// `bind_offset` is the starting `$N` parameter index (1-based).
    ///
    /// Returns `(sql_fragment, bind_values)`.
    /// When the filter is `Any`, returns `("TRUE", vec![])` — no binding needed.
    pub fn to_sql_predicate(&self, col: &str, bind_offset: usize) -> (String, Vec<String>) {
        match self {
            NamespaceFilter::Any => ("TRUE".to_string(), vec![]),
            NamespaceFilter::Exact(e) => (format!("{col} = ${bind_offset}"), vec![e.clone()]),
            NamespaceFilter::Prefix(p) => {
                (format!("{col} LIKE ${bind_offset}"), vec![format!("{p}%")])
            }
            NamespaceFilter::AnyOf(filters) => {
                if filters.is_empty() {
                    return ("TRUE".to_string(), vec![]);
                }
                let mut parts = Vec::new();
                let mut binds = Vec::new();
                let mut offset = bind_offset;
                for f in filters {
                    let (sql, vals) = f.to_sql_predicate(col, offset);
                    offset += vals.len();
                    parts.push(format!("({sql})"));
                    binds.extend(vals);
                }
                (parts.join(" OR "), binds)
            }
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

    #[test]
    fn sql_predicate_exact() {
        let f = NamespaceFilter::Exact("erp.po".into());
        let (sql, binds) = f.to_sql_predicate("namespace", 3);
        assert_eq!(sql, "namespace = $3");
        assert_eq!(binds, vec!["erp.po"]);
    }

    #[test]
    fn sql_predicate_prefix() {
        let f = NamespaceFilter::Prefix("accounting.".into());
        let (sql, binds) = f.to_sql_predicate("namespace", 1);
        assert_eq!(sql, "namespace LIKE $1");
        assert_eq!(binds, vec!["accounting.%"]);
    }

    #[test]
    fn sql_predicate_any() {
        let (sql, binds) = NamespaceFilter::Any.to_sql_predicate("namespace", 1);
        assert_eq!(sql, "TRUE");
        assert!(binds.is_empty());
    }
}
