//! Validation for capability manifests and tool slugs.

use crate::tools::manifest::ToolManifest;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RegisteredToolValidationError {
    #[error("invalid name '{0}': must match ^[a-z0-9-]{{2,64}}$")]
    InvalidName(String),
    #[error("manifest parse error: {0}")]
    ManifestParse(String),
    #[error("invalid JSON schema in {field}: {message}")]
    InvalidSchema { field: String, message: String },
    #[error("MCP endpoint disallowed: {0}")]
    McpEndpointDisallowed(String),
    #[error("WASM module rejected: {0}")]
    WasmRejected(String),
    #[error("size limit exceeded: {what} = {actual} > {limit}")]
    SizeLimit {
        what: &'static str,
        actual: usize,
        limit: usize,
    },
    #[error("[chain] section required for kind=chain without a bespoke Rust provider")]
    MissingChainSection,
    #[error("invalid namespace '{0}': must match ^[a-z][a-z0-9_]*(\\.[a-z][a-z0-9_]*){{0,5}}$")]
    InvalidNamespace(String),
}

#[derive(Debug, Default)]
pub struct ValidationReport {
    pub errors: Vec<RegisteredToolValidationError>,
    pub warnings: Vec<String>,
}

impl ValidationReport {
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }
}

pub struct RegisteredToolValidator;

impl RegisteredToolValidator {
    /// Validate a namespace string (dot-separated slugs, up to 6 segments).
    pub fn validate_namespace(ns: &str) -> ValidationReport {
        let mut r = ValidationReport::default();
        if ns.is_empty() {
            return r; // empty namespace is allowed (unnamespaced capability)
        }
        // Each segment: starts with [a-z], followed by [a-z0-9_]*
        let valid = ns.split('.').count() <= 6
            && ns.split('.').all(|seg| {
                !seg.is_empty()
                    && seg
                        .chars()
                        .next()
                        .map(|c| c.is_ascii_lowercase())
                        .unwrap_or(false)
                    && seg
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            });
        if !valid {
            r.errors
                .push(RegisteredToolValidationError::InvalidNamespace(
                    ns.to_string(),
                ));
        }
        r
    }

    /// Validate a capability name (slug).
    pub fn validate_name(name: &str) -> ValidationReport {
        let mut r = ValidationReport::default();
        let valid = name.len() >= 2
            && name.len() <= 64
            && name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if !valid {
            r.errors
                .push(RegisteredToolValidationError::InvalidName(name.to_string()));
        }
        r
    }

    /// Parse and validate a manifest TOML string.
    pub fn validate_manifest(toml: &str) -> ValidationReport {
        let mut r = ValidationReport::default();
        match ToolManifest::from_toml(toml) {
            Err(e) => r
                .errors
                .push(RegisteredToolValidationError::ManifestParse(e.to_string())),
            Ok(m) => {
                // Validate name.
                let name_r = Self::validate_name(&m.name);
                r.errors.extend(name_r.errors);
                r.warnings.extend(name_r.warnings);

                // Validate namespace if present.
                let ns_r = Self::validate_namespace(m.namespace());
                r.errors.extend(ns_r.errors);
                r.warnings.extend(ns_r.warnings);

                // Validate output_schema in [chain] if present.
                if let Some(chain) = &m.chain {
                    if let Some(schema) = &chain.output_schema
                        && !schema.is_object()
                    {
                        r.errors.push(RegisteredToolValidationError::InvalidSchema {
                            field: "chain.output_schema".into(),
                            message: "must be a JSON object (JSON Schema)".into(),
                        });
                    }
                    if chain.prompt_template.trim().is_empty() {
                        r.errors.push(RegisteredToolValidationError::ManifestParse(
                            "chain.prompt_template must not be empty".into(),
                        ));
                    }
                }

                // MCP endpoint allowlist check.
                if matches!(m.kind, crate::tools::manifest::ToolKind::Mcp) {
                    let allowed = std::env::var("CONUSAI_MCP_ALLOWED_HOSTS").unwrap_or_default();
                    if !allowed.is_empty() {
                        let endpoint = m.config["endpoint"].as_str().unwrap_or("");
                        let allowed_hosts: Vec<&str> = allowed.split(',').map(str::trim).collect();
                        let allowed = allowed_hosts.iter().any(|h| endpoint.contains(h));
                        if !allowed {
                            r.errors
                                .push(RegisteredToolValidationError::McpEndpointDisallowed(
                                    endpoint.to_string(),
                                ));
                        }
                    }
                }
            }
        }
        r
    }

    /// Validate WASM bytes: check magic bytes and size limit.
    pub fn validate_wasm(bytes: &[u8], max_bytes: usize) -> ValidationReport {
        let mut r = ValidationReport::default();
        if bytes.len() > max_bytes {
            r.errors.push(RegisteredToolValidationError::SizeLimit {
                what: "wasm",
                actual: bytes.len(),
                limit: max_bytes,
            });
        }
        if !bytes.starts_with(b"\0asm") {
            r.errors.push(RegisteredToolValidationError::WasmRejected(
                "missing WASM magic bytes \\0asm".into(),
            ));
        }
        r
    }

    /// Validate manifest TOML size.
    pub fn validate_manifest_size(toml: &str, max_bytes: usize) -> ValidationReport {
        let mut r = ValidationReport::default();
        if toml.len() > max_bytes {
            r.errors.push(RegisteredToolValidationError::SizeLimit {
                what: "manifest",
                actual: toml.len(),
                limit: max_bytes,
            });
        }
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_name() {
        assert!(RegisteredToolValidator::validate_name("my-tool-01").ok());
    }

    #[test]
    fn invalid_name_uppercase() {
        assert!(!RegisteredToolValidator::validate_name("My-Tool").ok());
    }

    #[test]
    fn invalid_name_too_short() {
        assert!(!RegisteredToolValidator::validate_name("x").ok());
    }

    #[test]
    fn valid_chain_manifest() {
        let toml = r#"
name = "test-tool"
version = "0.1.0"
description = "test"
kind = "chain"
tools = []
[chain]
model = "claude-opus-4-7"
prompt_template = "Summarise: {{input.text}}"
"#;
        assert!(RegisteredToolValidator::validate_manifest(toml).ok());
    }

    #[test]
    fn bad_manifest_parse() {
        let r = RegisteredToolValidator::validate_manifest("not valid toml {{{{");
        assert!(!r.ok());
    }

    #[test]
    fn valid_namespace_simple() {
        assert!(RegisteredToolValidator::validate_namespace("").ok());
        assert!(RegisteredToolValidator::validate_namespace("erp").ok());
        assert!(RegisteredToolValidator::validate_namespace("erp.po").ok());
        assert!(RegisteredToolValidator::validate_namespace("erp.po.create_order").ok());
    }

    #[test]
    fn invalid_namespace_uppercase() {
        assert!(!RegisteredToolValidator::validate_namespace("ERP.po").ok());
    }

    #[test]
    fn invalid_namespace_too_many_segments() {
        // 7 segments — exceeds limit of 6.
        assert!(!RegisteredToolValidator::validate_namespace("a.b.c.d.e.f.g").ok());
    }

    #[test]
    fn invalid_namespace_empty_segment() {
        assert!(!RegisteredToolValidator::validate_namespace("erp..po").ok());
    }

    #[test]
    fn invalid_namespace_starts_with_digit() {
        assert!(!RegisteredToolValidator::validate_namespace("1erp").ok());
    }
}
