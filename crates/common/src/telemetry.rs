//! Telemetry bootstrap: structured JSON logging + OTel traces + OTel metrics.
//!
//! # Environment variables
//!
//! | Variable                          | Default      | Purpose                                |
//! |-----------------------------------|--------------|----------------------------------------|
//! | `OTEL_EXPORTER_OTLP_ENDPOINT`     | —            | gRPC OTLP endpoint (enables OTel)      |
//! | `OTLP_ENDPOINT`                   | —            | Alias for the above (legacy)           |
//! | `OTEL_SERVICE_NAME`               | `service_name` arg | Overrides the service name        |
//! | `OTEL_RESOURCE_ATTRIBUTES`        | —            | Extra `key=value,…` resource attrs     |
//! | `OTEL_TRACES_SAMPLER`             | `always_on`  | `always_on` | `always_off` | `traceidratio` |
//! | `OTEL_TRACES_SAMPLER_ARG`         | `1.0`        | Sampling ratio when using traceidratio |
//! | `OTEL_EXPORTER_OTLP_TIMEOUT_MILLIS` | `10000`    | OTLP export timeout                    |
//! | `RUST_LOG`                        | `log_level`  | Overrides log filter                   |
//! | `DEPLOY_ENV`                      | `development`| Sets `deployment.environment` resource |

use opentelemetry::KeyValue;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    Resource,
    metrics::{PeriodicReader, SdkMeterProvider},
    propagation::TraceContextPropagator,
    runtime,
    trace::{RandomIdGenerator, Sampler, TracerProvider},
};
use prometheus::Registry;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

/// Held by `main()` — flushes and shuts down both OTLP pipelines on drop.
pub struct TelemetryGuard {
    tracer_provider: Option<TracerProvider>,
    meter_provider: Option<SdkMeterProvider>,
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if let Some(p) = self.tracer_provider.take() {
            let _ = p.shutdown();
        }
        if let Some(m) = self.meter_provider.take() {
            let _ = m.shutdown();
        }
        opentelemetry::global::shutdown_tracer_provider();
    }
}

/// Build the OTel `Resource` from env + hard-coded semantic convention attrs.
fn build_resource(service_name: &str) -> Resource {
    let svc_name = std::env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| service_name.to_owned());
    let deploy_env =
        std::env::var("DEPLOY_ENV").unwrap_or_else(|_| "development".to_owned());

    let mut attrs = vec![
        KeyValue::new("service.name", svc_name),
        KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
        KeyValue::new("deployment.environment", deploy_env),
        KeyValue::new("telemetry.sdk.language", "rust"),
        KeyValue::new("telemetry.sdk.name", "opentelemetry"),
    ];

    // Add hostname as instance ID when available.
    if let Ok(h) = std::env::var("HOSTNAME").or_else(|_| {
        std::fs::read_to_string("/etc/hostname").map(|s| s.trim().to_owned())
    }) {
        attrs.push(KeyValue::new("service.instance.id", h));
    }

    // OTEL_RESOURCE_ATTRIBUTES=key1=val1,key2=val2
    if let Ok(extra) = std::env::var("OTEL_RESOURCE_ATTRIBUTES") {
        for pair in extra.split(',') {
            if let Some((k, v)) = pair.split_once('=') {
                attrs.push(KeyValue::new(k.trim().to_owned(), v.trim().to_owned()));
            }
        }
    }

    Resource::new(attrs)
}

/// Resolve the OTLP endpoint from either the standard env var or the legacy alias.
fn otlp_endpoint() -> Option<String> {
    std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .or_else(|_| std::env::var("OTLP_ENDPOINT"))
        .ok()
}

/// Build the trace sampler from `OTEL_TRACES_SAMPLER` / `OTEL_TRACES_SAMPLER_ARG`.
fn build_sampler() -> Sampler {
    let ratio: f64 = std::env::var("OTEL_TRACES_SAMPLER_ARG")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(1.0)
        .clamp(0.0, 1.0);

    match std::env::var("OTEL_TRACES_SAMPLER")
        .unwrap_or_else(|_| "always_on".into())
        .as_str()
    {
        "always_off" => Sampler::AlwaysOff,
        "traceidratio" => Sampler::TraceIdRatioBased(ratio),
        "parentbased_always_on" => Sampler::ParentBased(Box::new(Sampler::AlwaysOn)),
        "parentbased_always_off" => Sampler::ParentBased(Box::new(Sampler::AlwaysOff)),
        "parentbased_traceidratio" => {
            Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(ratio)))
        }
        _ => Sampler::AlwaysOn,
    }
}

/// Initialise structured JSON logging + optional OTLP trace + metrics export.
///
/// Returns a [`TelemetryGuard`] that **must** be held until process exit.
/// Also returns a Prometheus [`Registry`] that serves the `/metrics` endpoint.
pub fn init(service_name: &str, log_level: &str) -> (TelemetryGuard, Registry) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level));

    let fmt_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true);

    // ── Prometheus registry (always active, even without OTLP) ───────────────
    let prom_registry = Registry::new();
    let prom_exporter = opentelemetry_prometheus::exporter()
        .with_registry(prom_registry.clone())
        .build()
        .expect("build Prometheus exporter");

    let mut meter_builder = SdkMeterProvider::builder()
        .with_reader(prom_exporter)
        .with_resource(build_resource(service_name));

    let otlp = otlp_endpoint();
    let mut tracer_provider_opt = None;
    let mut otel_layer_opt = None;

    if let Some(endpoint) = &otlp {
        // ── Traces ────────────────────────────────────────────────────────────
        let span_exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .build()
            .unwrap_or_else(|e| panic!("OTLP span exporter ({endpoint}): {e}"));

        let tracer_provider = TracerProvider::builder()
            .with_batch_exporter(span_exporter, runtime::Tokio)
            .with_resource(build_resource(service_name))
            .with_sampler(build_sampler())
            .with_id_generator(RandomIdGenerator::default())
            .build();

        opentelemetry::global::set_tracer_provider(tracer_provider.clone());
        opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());

        otel_layer_opt = Some(
            tracing_opentelemetry::layer()
                .with_tracer(tracer_provider.tracer(service_name.to_owned()))
                .with_error_fields_to_exceptions(true),
        );
        tracer_provider_opt = Some(tracer_provider);

        // ── Metrics over OTLP (in addition to Prometheus) ─────────────────────
        if let Ok(mx) = opentelemetry_otlp::MetricExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .build()
        {
            let reader = PeriodicReader::builder(mx, runtime::Tokio).build();
            meter_builder = meter_builder.with_reader(reader);
        }
    }

    let meter_provider = meter_builder.build();
    opentelemetry::global::set_meter_provider(meter_provider.clone());

    let subscriber = tracing_subscriber::registry().with(filter).with(fmt_layer);
    if let Some(otel_layer) = otel_layer_opt {
        subscriber.with(otel_layer).init();
    } else {
        subscriber.init();
    }

    (
        TelemetryGuard {
            tracer_provider: tracer_provider_opt,
            meter_provider: Some(meter_provider),
        },
        prom_registry,
    )
}

/// Convenience: create a named meter from the global provider.
/// Use this in any crate that needs counters / histograms.
pub fn meter(name: &'static str) -> opentelemetry::metrics::Meter {
    opentelemetry::global::meter(name)
}
