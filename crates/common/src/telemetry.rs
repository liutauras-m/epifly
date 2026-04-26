use opentelemetry::KeyValue;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    Resource, propagation::TraceContextPropagator, runtime, trace::TracerProvider,
};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

/// Held by main() — flushes and shuts down the OTLP pipeline on drop.
pub struct TelemetryGuard {
    provider: Option<TracerProvider>,
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if let Some(p) = self.provider.take() {
            let _ = p.shutdown();
        }
        opentelemetry::global::shutdown_tracer_provider();
    }
}

/// Initialise structured JSON logging + optional OTLP trace export.
///
/// Set `OTLP_ENDPOINT=http://localhost:4317` to enable trace export.
/// Set `RUST_LOG` to override `log_level`.
pub fn init(service_name: &str, log_level: &str) -> TelemetryGuard {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level));

    let fmt_layer = tracing_subscriber::fmt::layer().json();

    if let Ok(endpoint) = std::env::var("OTLP_ENDPOINT") {
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .build()
            .expect("build OTLP span exporter");

        let resource = Resource::new(vec![KeyValue::new("service.name", service_name.to_owned())]);

        let provider = TracerProvider::builder()
            .with_batch_exporter(exporter, runtime::Tokio)
            .with_resource(resource)
            .build();

        opentelemetry::global::set_tracer_provider(provider.clone());
        opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());

        let otel_layer =
            tracing_opentelemetry::layer().with_tracer(provider.tracer(service_name.to_owned()));

        tracing_subscriber::registry()
            .with(filter)
            .with(fmt_layer)
            .with(otel_layer)
            .init();

        TelemetryGuard {
            provider: Some(provider),
        }
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt_layer)
            .init();

        TelemetryGuard { provider: None }
    }
}
