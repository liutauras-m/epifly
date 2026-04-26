//! Shared OpenTelemetry metric definitions.
//!
//! All meters are lazily initialised on first use so they work even if called
//! before the global meter provider is configured (e.g. in unit tests).
//!
//! Import `record_error` to attach error context to the current span, following
//! the OpenTelemetry semantic conventions for exceptions.

use opentelemetry::{
    KeyValue,
    metrics::{Counter, Histogram},
};
use tracing::Span;

// ── Span error recording ──────────────────────────────────────────────────────

/// Record an error on the current span using OTel semantic conventions.
///
/// Sets `error.type` and emits an `exception` event with the message.
pub fn record_error(span: &Span, err: &dyn std::fmt::Display) {
    span.record("error.type", err.to_string().as_str());
    tracing::error!(parent: span, error = %err, "span error");
}

// ── Tool / capability metrics ─────────────────────────────────────────────────

/// Counter for total tool invocations, labelled by capability and tool name.
pub fn tool_invocations() -> Counter<u64> {
    opentelemetry::global::meter("conusai.agent")
        .u64_counter("agent.tool.invocations")
        .with_description("Total tool invocations by capability and tool")
        .with_unit("invocations")
        .build()
}

/// Counter for failed tool invocations.
pub fn tool_errors() -> Counter<u64> {
    opentelemetry::global::meter("conusai.agent")
        .u64_counter("agent.tool.errors")
        .with_description("Tool invocations that returned an error")
        .with_unit("errors")
        .build()
}

/// Histogram of tool execution duration.
pub fn tool_duration_ms() -> Histogram<f64> {
    opentelemetry::global::meter("conusai.agent")
        .f64_histogram("agent.tool.duration_ms")
        .with_description("Tool execution wall-clock time in milliseconds")
        .with_unit("ms")
        .build()
}

// ── LLM / agent completions metrics ──────────────────────────────────────────

/// Counter for agent completion requests.
pub fn llm_requests() -> Counter<u64> {
    opentelemetry::global::meter("conusai.agent")
        .u64_counter("agent.llm.requests")
        .with_description("Total LLM completion requests")
        .with_unit("requests")
        .build()
}

/// Histogram of input token counts.
pub fn llm_input_tokens() -> Histogram<u64> {
    opentelemetry::global::meter("conusai.agent")
        .u64_histogram("agent.llm.input_tokens")
        .with_description("LLM prompt input token count")
        .with_unit("tokens")
        .build()
}

/// Histogram of output token counts.
pub fn llm_output_tokens() -> Histogram<u64> {
    opentelemetry::global::meter("conusai.agent")
        .u64_histogram("agent.llm.output_tokens")
        .with_description("LLM completion output token count")
        .with_unit("tokens")
        .build()
}

// ── Qdrant / storage metrics ──────────────────────────────────────────────────

/// Histogram of Qdrant REST request durations.
pub fn qdrant_duration_ms() -> Histogram<f64> {
    opentelemetry::global::meter("conusai.storage")
        .f64_histogram("qdrant.request.duration_ms")
        .with_description("Qdrant REST request duration in milliseconds")
        .with_unit("ms")
        .build()
}

/// Counter for Qdrant errors, labelled by operation.
pub fn qdrant_errors() -> Counter<u64> {
    opentelemetry::global::meter("conusai.storage")
        .u64_counter("qdrant.request.errors")
        .with_description("Qdrant REST requests that returned an error")
        .with_unit("errors")
        .build()
}

// ── Convenience label constructors ───────────────────────────────────────────

pub fn kv(k: &'static str, v: impl Into<String>) -> KeyValue {
    KeyValue::new(k, v.into())
}
