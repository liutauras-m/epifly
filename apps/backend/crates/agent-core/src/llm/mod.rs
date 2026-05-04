//! LLM abstraction layer — single source of truth for all model access.
//!
//! Swapping providers or adding a new provider requires a change in exactly one
//! place (`llm/providers/`).  No route, chain, or memory module should
//! construct a provider client directly.
//!
//! ## Module map
//!
//! - [`types`] — `LlmRequest`, `LlmResponse`, `LlmStream`, `LlmBinding`, …
//! - [`error`] — `LlmError`
//! - [`provider`] — `LlmProvider` trait
//! - [`registry`] — `LlmRegistry` + `verify_llm_providers`
//! - [`streaming`] — OpenAI-compatible SSE helper
//! - [`providers`] — concrete provider implementations

pub mod error;
pub mod provider;
pub mod providers;
pub mod registry;
pub mod streaming;
pub mod types;

// ── Flat re-exports ───────────────────────────────────────────────────────────

pub use error::LlmError;
pub use provider::LlmProvider;
pub use registry::{LlmRegistry, verify_llm_providers};
pub use types::{
    LlmBinding, LlmChunk, LlmRequest, LlmResponse, LlmStream, LlmUsage,
};
