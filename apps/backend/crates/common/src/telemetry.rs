//! Telemetry bootstrap: structured JSON logs on stdout, a Prometheus registry,
//! and — when `OTEL_EXPORTER_OTLP_ENDPOINT` is set — OTLP export of traces,
//! metrics and logs over **HTTP/protobuf**.
//!
//! The exporter reads the standard OpenTelemetry variables itself:
//!
//! | Variable                      | Meaning                                                        |
//! |-------------------------------|----------------------------------------------------------------|
//! | `OTEL_EXPORTER_OTLP_ENDPOINT` | Base URL; `/v1/traces`, `/v1/metrics`, `/v1/logs` are appended |
//! | `OTEL_EXPORTER_OTLP_HEADERS`  | `k=v,k2=v2` (values URL-encoded), e.g. `Authorization=Bearer%20dt_…` |
//! | `OTEL_SERVICE_NAME`           | Overrides the service name                                     |
//! | `OTEL_RESOURCE_ATTRIBUTES`    | Extra `key=value,…` resource attributes                        |
//! | `OTEL_TRACES_SAMPLER` / `_ARG`| Sampler (SDK default: parent-based always-on)                  |
//! | `RUST_LOG`                    | Log filter (defaults to the `log_level` argument)              |
//!
//! There is no gRPC exporter: the ConusAI Cloud ingest (`<base>/otel/v1/*`) speaks
//! OTLP/HTTP only. With the endpoint unset, telemetry stays local (stdout + `/metrics`).

use opentelemetry::KeyValue;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_sdk::{
    Resource, logs::SdkLoggerProvider, metrics::PeriodicReader, metrics::SdkMeterProvider,
    propagation::TraceContextPropagator, trace::SdkTracerProvider,
};
use prometheus::Registry;
use tracing_subscriber::{
    EnvFilter, Layer, filter::filter_fn, layer::SubscriberExt, util::SubscriberInitExt,
};

/// Held by `main()` — flushes and shuts down the OTLP pipelines on drop.
pub struct TelemetryGuard {
    tracer_provider: Option<SdkTracerProvider>,
    meter_provider: Option<SdkMeterProvider>,
    logger_provider: Option<SdkLoggerProvider>,
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if let Some(p) = self.tracer_provider.take() {
            let _ = p.shutdown();
        }
        if let Some(m) = self.meter_provider.take() {
            let _ = m.shutdown();
        }
        if let Some(l) = self.logger_provider.take() {
            let _ = l.shutdown();
        }
    }
}

/// The OTLP base endpoint, when telemetry export is enabled.
pub fn otlp_endpoint() -> Option<String> {
    std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .ok()
        .filter(|v| !v.trim().is_empty())
}

/// Resource from the SDK detectors (which read `OTEL_SERVICE_NAME` and
/// `OTEL_RESOURCE_ATTRIBUTES`), defaulting the service name to `service_name`.
fn build_resource(service_name: &str) -> Resource {
    let mut builder = Resource::builder()
        .with_attribute(KeyValue::new("service.version", env!("CARGO_PKG_VERSION")));
    if std::env::var("OTEL_SERVICE_NAME").is_err() {
        builder = builder.with_service_name(service_name.to_owned());
    }
    if let Ok(h) = std::env::var("HOSTNAME") {
        builder = builder.with_attribute(KeyValue::new("service.instance.id", h));
    }
    builder.build()
}

/// Events emitted by the exporter stack itself must not be fed back into the
/// OTLP log pipeline (that would loop on every export).
fn is_exporter_internal(target: &str) -> bool {
    ["opentelemetry", "hyper", "reqwest", "h2", "tower", "rustls"]
        .iter()
        .any(|p| target.starts_with(p))
}

/// Initialise logging and telemetry. Returns a guard that must live until
/// process exit, and the Prometheus registry served at `/metrics`.
///
/// Fails when the OTLP endpoint is set but an exporter cannot be built — a
/// misconfigured telemetry endpoint is a startup error, not a silent no-op.
pub fn init(service_name: &str, log_level: &str) -> anyhow::Result<(TelemetryGuard, Registry)> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level));
    let fmt_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_target(true)
        .with_file(true)
        .with_line_number(true);

    // Build the Prometheus exporter exactly once per registry (a second build
    // fails with "Duplicate metrics collector registration attempted").
    let prom_registry = Registry::new();
    let prom_exporter = opentelemetry_prometheus::exporter()
        .with_registry(prom_registry.clone())
        .build()?;

    let resource = build_resource(service_name);

    let Some(endpoint) = otlp_endpoint() else {
        let meter_provider = SdkMeterProvider::builder()
            .with_reader(prom_exporter)
            .with_resource(resource)
            .build();
        opentelemetry::global::set_meter_provider(meter_provider.clone());
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt_layer)
            .init();
        return Ok((
            TelemetryGuard {
                tracer_provider: None,
                meter_provider: Some(meter_provider),
                logger_provider: None,
            },
            prom_registry,
        ));
    };

    // The exporters resolve `<endpoint>/v1/<signal>`, the headers and the sampler
    // from the standard env variables themselves.
    let span_exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .build()
        .map_err(|e| anyhow::anyhow!("OTLP span exporter ({endpoint}): {e}"))?;
    let metric_exporter = opentelemetry_otlp::MetricExporter::builder()
        .with_http()
        .build()
        .map_err(|e| anyhow::anyhow!("OTLP metric exporter ({endpoint}): {e}"))?;
    let log_exporter = opentelemetry_otlp::LogExporter::builder()
        .with_http()
        .build()
        .map_err(|e| anyhow::anyhow!("OTLP log exporter ({endpoint}): {e}"))?;

    let tracer_provider = SdkTracerProvider::builder()
        .with_batch_exporter(span_exporter)
        .with_resource(resource.clone())
        .build();
    opentelemetry::global::set_tracer_provider(tracer_provider.clone());
    opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());

    let meter_provider = SdkMeterProvider::builder()
        .with_reader(prom_exporter)
        .with_reader(PeriodicReader::builder(metric_exporter).build())
        .with_resource(resource.clone())
        .build();
    opentelemetry::global::set_meter_provider(meter_provider.clone());

    let logger_provider = SdkLoggerProvider::builder()
        .with_batch_exporter(log_exporter)
        .with_resource(resource)
        .build();

    let otel_trace_layer = tracing_opentelemetry::layer()
        .with_tracer(tracer_provider.tracer(service_name.to_owned()))
        .with_error_fields_to_exceptions(true);
    let otel_log_layer = OpenTelemetryTracingBridge::new(&logger_provider)
        .with_filter(filter_fn(|meta| !is_exporter_internal(meta.target())));

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .with(otel_trace_layer)
        .with(otel_log_layer)
        .init();

    Ok((
        TelemetryGuard {
            tracer_provider: Some(tracer_provider),
            meter_provider: Some(meter_provider),
            logger_provider: Some(logger_provider),
        },
        prom_registry,
    ))
}

/// Convenience: create a named meter from the global provider.
pub fn meter(name: &'static str) -> opentelemetry::metrics::Meter {
    opentelemetry::global::meter(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exporter_internal_targets_are_filtered() {
        assert!(is_exporter_internal("opentelemetry_sdk::logs"));
        assert!(is_exporter_internal("hyper_util::client"));
        assert!(is_exporter_internal("reqwest::connect"));
        assert!(!is_exporter_internal("agent_gateway::routes::agent"));
    }
}
