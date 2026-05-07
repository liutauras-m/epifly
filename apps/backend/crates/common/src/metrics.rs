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

// ── Database metrics ──────────────────────────────────────────────────────────

/// Histogram of database query durations.
pub fn db_query_duration_ms() -> Histogram<f64> {
    opentelemetry::global::meter("conusai.storage")
        .f64_histogram("db.query.duration_ms")
        .with_description("Postgres query duration in milliseconds")
        .with_unit("ms")
        .build()
}

/// Counter for database errors, labelled by operation.
pub fn db_errors() -> Counter<u64> {
    opentelemetry::global::meter("conusai.storage")
        .u64_counter("db.query.errors")
        .with_description("Postgres queries that returned an error")
        .with_unit("errors")
        .build()
}

// ── Convenience label constructors ───────────────────────────────────────────

pub fn kv(k: &'static str, v: impl Into<String>) -> KeyValue {
    KeyValue::new(k, v.into())
}

// ── Semantic router metrics (OTel GenAI conventions) ─────────────────────────

/// Counter: cache hits in the semantic router.
pub fn semantic_router_cache_hits() -> Counter<u64> {
    opentelemetry::global::meter("conusai.agent")
        .u64_counter("gen_ai.semantic_router.cache_hit")
        .with_description("Semantic router embedding cache hits")
        .with_unit("hits")
        .build()
}

/// Histogram: top-K size returned by the semantic router.
pub fn semantic_router_top_k() -> Histogram<u64> {
    opentelemetry::global::meter("conusai.agent")
        .u64_histogram("gen_ai.semantic_router.top_k")
        .with_description("Number of capabilities selected by the semantic router per turn")
        .with_unit("capabilities")
        .build()
}

/// Histogram: cosine distance of best (closest) hit.
pub fn semantic_router_distance() -> Histogram<f64> {
    opentelemetry::global::meter("conusai.agent")
        .f64_histogram("gen_ai.semantic_router.distance")
        .with_description("Cosine distance of the top-1 capability hit (lower = closer)")
        .build()
}

/// Counter: total tool calls tracked by the GenAI semantic conventions.
pub fn gen_ai_tool_calls() -> Counter<u64> {
    opentelemetry::global::meter("conusai.agent")
        .u64_counter("gen_ai.tool.calls")
        .with_description("Total tool calls dispatched via the semantic router")
        .with_unit("calls")
        .build()
}

/// Histogram: semantic router select latency.
pub fn capability_router_select_seconds() -> Histogram<f64> {
    opentelemetry::global::meter("conusai.agent")
        .f64_histogram("capability_router_select_seconds")
        .with_description("Time to resolve top-K capabilities in the semantic router")
        .with_unit("s")
        .build()
}

/// Histogram: capability invoke latency.
pub fn capability_invoke_seconds() -> Histogram<f64> {
    opentelemetry::global::meter("conusai.agent")
        .f64_histogram("capability_invoke_seconds")
        .with_description("Time to invoke a capability provider")
        .with_unit("s")
        .build()
}
