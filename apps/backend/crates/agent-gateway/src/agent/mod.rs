//! Agent module — Step 2.1.
//!
//! Extracted from `routes/agent.rs`. Route handlers now use these types directly;
//! the 2253-line monolith shrinks to ~200 lines of HTTP wiring.

pub mod context;
pub mod metering;
pub mod persistence;
pub mod provider;
pub mod runner;
pub mod tool_execution;

// Re-export the main public API for route handlers.
pub use context::{AgentCtx, build_ctx, merge_pinned};
pub use provider::anthropic::NativeAnthropicProvider;
pub use runner::{AgentEmitError, AgentError, AgentEvent, AgentEventSink, AgentTurnRunner, BlockingSink, SseSink};
