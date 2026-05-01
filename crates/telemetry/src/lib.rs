use std::error::Error;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub type InitError = Box<dyn Error + 'static>;

#[cfg(feature = "metrics")]
pub use metrics_exporter_prometheus::PrometheusHandle as MetricsHandle;

#[cfg(not(feature = "metrics"))]
#[derive(Clone, Debug)]
pub struct MetricsHandle;

pub struct TelemetryGuard {
    #[cfg(feature = "metrics")]
    metrics: Option<MetricsHandle>,
}

impl TelemetryGuard {
    #[cfg(feature = "metrics")]
    pub fn metrics_handle(&self) -> Option<MetricsHandle> {
        self.metrics.clone()
    }

    #[cfg(not(feature = "metrics"))]
    pub fn metrics_handle(&self) -> Option<MetricsHandle> {
        None
    }
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        #[cfg(feature = "otel")]
        opentelemetry::global::shutdown_tracer_provider();
    }
}

pub fn init(service_name: &str) -> Result<TelemetryGuard, InitError> {
    #[cfg(feature = "metrics")]
    let metrics = Some(metrics_exporter_prometheus::PrometheusBuilder::new().install_recorder()?);

    init_tracing(service_name)?;

    Ok(TelemetryGuard {
        #[cfg(feature = "metrics")]
        metrics,
    })
}

fn env_filter() -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
}

#[cfg(not(feature = "otel"))]
fn init_tracing(_service_name: &str) -> Result<(), InitError> {
    tracing_subscriber::registry()
        .with(env_filter())
        .with(tracing_subscriber::fmt::layer().json().flatten_event(true))
        .try_init()?;
    Ok(())
}

#[cfg(feature = "otel")]
fn init_tracing(service_name: &str) -> Result<(), InitError> {
    use opentelemetry::KeyValue;
    use opentelemetry_sdk::Resource;

    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(opentelemetry_otlp::new_exporter().tonic())
        .with_trace_config(
            opentelemetry_sdk::trace::config().with_resource(Resource::new(vec![KeyValue::new(
                "service.name",
                service_name.to_owned(),
            )])),
        )
        .install_batch(opentelemetry_sdk::runtime::Tokio)?;

    tracing_subscriber::registry()
        .with(env_filter())
        .with(tracing_subscriber::fmt::layer().json().flatten_event(true))
        .with(tracing_opentelemetry::layer().with_tracer(tracer))
        .try_init()?;
    Ok(())
}
