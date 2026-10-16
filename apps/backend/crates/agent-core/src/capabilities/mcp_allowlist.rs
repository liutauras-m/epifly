//! Exact-host allowlist for every outbound MCP endpoint.
//!
//! `CONUSAI_MCP_ALLOWED_HOSTS` is a comma-separated list of hosts (`mcp.example.com`)
//! or host:port pairs (`ocr:8080`). A URL is allowed only when its scheme is
//! http(s), it carries no userinfo, and its host — or host:port — equals an entry
//! (case-insensitive). An empty or unset list denies every remote MCP endpoint.
//!
//! The check runs whenever an MCP adapter is built (`McpAdapter::new`), so it covers
//! self-registered, admin-uploaded, on-disk and hot-reloaded manifests alike.

use url::Url;

pub const ENV_VAR: &str = "CONUSAI_MCP_ALLOWED_HOSTS";

#[derive(Debug, Clone, Default)]
pub struct McpAllowlist {
    entries: Vec<String>,
}

impl McpAllowlist {
    pub fn parse(raw: &str) -> Self {
        Self {
            entries: raw
                .split(',')
                .map(|e| e.trim().to_ascii_lowercase())
                .filter(|e| !e.is_empty())
                .collect(),
        }
    }

    /// The allowlist currently configured in the environment.
    pub fn from_env() -> Self {
        Self::parse(&std::env::var(ENV_VAR).unwrap_or_default())
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Parse `endpoint` and check it against the list.
    pub fn check(&self, endpoint: &str) -> Result<Url, String> {
        let url =
            Url::parse(endpoint).map_err(|e| format!("invalid MCP endpoint {endpoint:?}: {e}"))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(format!(
                "MCP endpoint scheme {} is not allowed",
                url.scheme()
            ));
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err("MCP endpoint must not carry credentials in the URL".into());
        }
        let host = url
            .host_str()
            .ok_or_else(|| format!("MCP endpoint {endpoint:?} has no host"))?
            .to_ascii_lowercase();
        let host_port = url.port().map(|p| format!("{host}:{p}"));
        let allowed = self
            .entries
            .iter()
            .any(|e| *e == host || host_port.as_deref() == Some(e.as_str()));
        if allowed {
            Ok(url)
        } else if self.entries.is_empty() {
            Err(format!(
                "remote MCP is disabled ({ENV_VAR} is empty); refused {host}"
            ))
        } else {
            Err(format!("MCP host {host} is not in {ENV_VAR}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_list_denies_everything() {
        let l = McpAllowlist::parse("");
        assert!(l.check("https://mcp.example.com/rpc").is_err());
    }

    #[test]
    fn exact_host_match_only() {
        let l = McpAllowlist::parse("mcp.example.com, ocr:8080");
        assert!(l.check("https://mcp.example.com/rpc").is_ok());
        assert!(l.check("https://MCP.example.com/rpc").is_ok());
        assert!(l.check("http://ocr:8080/mcp").is_ok());
        // Substring / suffix tricks are refused.
        assert!(l.check("https://mcp.example.com.evil.net/rpc").is_err());
        assert!(l.check("https://evil-mcp.example.com/rpc").is_err());
        assert!(l.check("https://evil.net/?mcp.example.com").is_err());
        // host:port entries pin the port.
        assert!(l.check("http://ocr:9090/mcp").is_err());
    }

    #[test]
    fn rejects_credentials_and_odd_schemes() {
        let l = McpAllowlist::parse("mcp.example.com");
        assert!(l.check("https://user:pw@mcp.example.com/").is_err());
        assert!(l.check("file:///etc/passwd").is_err());
        assert!(l.check("${S3_ENDPOINT}").is_err());
    }
}
