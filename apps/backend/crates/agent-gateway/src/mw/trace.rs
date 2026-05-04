/// W3C Trace Context propagation — extracts `traceparent` / `tracestate` from
/// incoming HTTP headers and sets them as the parent on the current span.
use axum::{extract::Request, middleware::Next, response::Response};
use opentelemetry::propagation::{Extractor, TextMapPropagator};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use tracing_opentelemetry::OpenTelemetrySpanExt;

struct HeaderExtractor<'a>(&'a axum::http::HeaderMap);

impl<'a> Extractor for HeaderExtractor<'a> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|v| v.to_str().ok())
    }
    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|k| k.as_str()).collect()
    }
}

pub async fn propagate_trace(req: Request, next: Next) -> Response {
    let parent_cx = TraceContextPropagator::new().extract(&HeaderExtractor(req.headers()));
    tracing::Span::current().set_parent(parent_cx);
    next.run(req).await
}
