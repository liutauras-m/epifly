//! Centralized session authentication for the agent-gateway.
//!
//! All auth logic lives here. No other module should duplicate cookie parsing,
//! HMAC verification, or X-Session-Token extraction.
//!
//! # Structure
//! - [`verifier`] — HMAC-SHA256 token validation, `SessionUser` struct.
//! - [`extractor`] — multi-vector header extraction + Axum `FromRequestParts` impl.
//!
//! # Usage
//! ```rust
//! // In a handler (via FromRequestParts):
//! async fn my_handler(user: SessionUser, ...) { ... }
//!
//! // In middleware (no extractor overhead):
//! if let Some(user) = crate::auth::extract_from_headers(req.headers()) { ... }
//! ```

mod extractor;
mod verifier;

pub use verifier::{COOKIE_NAME, SessionUser, verify};
pub use extractor::extract_from_headers;
