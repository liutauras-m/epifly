pub mod config;
pub mod error;
pub mod memory;
pub mod eval;
pub mod http_client;
pub mod limits;
pub mod mcp;
pub mod path_safety;
pub mod telemetry;
pub mod wasm;

pub mod prelude {
    pub use crate::error::{ConusAiError, Result};
    pub use tracing::{debug, error, info, instrument, warn};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_join_valid() {
        let base = std::path::Path::new("/tmp/capabilities");
        let result = path_safety::safe_join(base, "invoice-processing");
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            std::path::PathBuf::from("/tmp/capabilities/invoice-processing")
        );
    }

    #[test]
    fn test_safe_join_traversal_rejected() {
        let base = std::path::Path::new("/tmp/capabilities");
        let result = path_safety::safe_join(base, "../../etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn test_mcp_request_serialization() {
        let req = mcp::JsonRpcRequest::new("tools/list", None);
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("tools/list"));
        assert!(json.contains("2.0"));
    }

    #[test]
    fn test_api_error_fields() {
        let e = error::ApiError::new(404, "not found");
        assert_eq!(e.code, 404);
        assert_eq!(e.message, "not found");
    }

    #[test]
    fn test_limits_sanity() {
        assert!(limits::MAX_PROMPT_TOKENS > 0);
        assert!(limits::MAX_WASM_SIZE_BYTES < limits::MAX_CAPABILITY_SIZE_BYTES);
    }
}
