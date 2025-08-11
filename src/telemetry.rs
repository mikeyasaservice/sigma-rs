//! OpenTelemetry integration for distributed tracing
//!
//! This module provides OpenTelemetry support for sigma-rs, enabling
//! distributed tracing across your security event processing pipeline.

use anyhow::Result;
use opentelemetry::global;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::{
    trace::{RandomIdGenerator, Sampler},
    Resource,
};
use opentelemetry_semantic_conventions::resource::{
    DEPLOYMENT_ENVIRONMENT, SERVICE_NAME, SERVICE_VERSION,
};
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};

/// OpenTelemetry configuration
#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    /// OTLP endpoint (e.g., "http://localhost:4317")
    pub endpoint: String,
    /// Service name
    pub service_name: String,
    /// Service version
    pub service_version: String,
    /// Deployment environment (e.g., "production", "staging")
    pub environment: String,
    /// Sampling rate (0.0 to 1.0)
    pub sampling_rate: f64,
    /// Enable trace propagation
    pub propagation: bool,
    /// Additional resource attributes
    pub resource_attributes: Vec<(String, String)>,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:4317".to_string(),
            service_name: "sigma-rs".to_string(),
            service_version: env!("CARGO_PKG_VERSION").to_string(),
            environment: "development".to_string(),
            sampling_rate: 0.1,
            propagation: true,
            resource_attributes: vec![],
        }
    }
}

/// Initialize OpenTelemetry with the provided configuration
pub fn init_telemetry(config: TelemetryConfig) -> Result<()> {
    // Set up propagator
    if config.propagation {
        global::set_text_map_propagator(TraceContextPropagator::new());
    }

    // Create OTLP exporter
    let exporter = opentelemetry_otlp::new_exporter()
        .tonic()
        .with_endpoint(&config.endpoint);

    // Set up resource
    let mut resource_kvs = vec![
        opentelemetry::KeyValue::new(SERVICE_NAME, config.service_name.clone()),
        opentelemetry::KeyValue::new(SERVICE_VERSION, config.service_version.clone()),
        opentelemetry::KeyValue::new(DEPLOYMENT_ENVIRONMENT, config.environment.clone()),
    ];

    // Add custom resource attributes
    for (key, value) in config.resource_attributes {
        resource_kvs.push(opentelemetry::KeyValue::new(key, value));
    }

    let resource = Resource::new(resource_kvs);

    // Create tracer
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(exporter)
        .with_trace_config(
            opentelemetry_sdk::trace::Config::default()
                .with_sampler(Sampler::TraceIdRatioBased(config.sampling_rate))
                .with_id_generator(RandomIdGenerator::default())
                .with_resource(resource),
        )
        .install_batch(opentelemetry_sdk::runtime::Tokio)?;

    // Create telemetry layer
    let telemetry_layer = OpenTelemetryLayer::new(tracer);

    // Set up subscriber with telemetry layer
    let subscriber = Registry::default()
        .with(EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .with(telemetry_layer);

    subscriber.init();

    Ok(())
}

/// Initialize telemetry from environment variables
pub fn init_telemetry_from_env() -> Result<()> {
    let config = TelemetryConfig {
        endpoint: std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
            .unwrap_or_else(|_| "http://localhost:4317".to_string()),
        service_name: std::env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| "sigma-rs".to_string()),
        service_version: std::env::var("OTEL_SERVICE_VERSION")
            .unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string()),
        environment: std::env::var("DEPLOYMENT_ENVIRONMENT")
            .unwrap_or_else(|_| "development".to_string()),
        sampling_rate: std::env::var("OTEL_TRACES_SAMPLER_ARG")
            .unwrap_or_else(|_| "0.1".to_string())
            .parse()
            .unwrap_or(0.1),
        propagation: std::env::var("OTEL_PROPAGATION_ENABLED")
            .unwrap_or_else(|_| "true".to_string())
            .parse()
            .unwrap_or(true),
        resource_attributes: vec![],
    };

    init_telemetry(config)
}

/// Shutdown OpenTelemetry gracefully
pub fn shutdown_telemetry() {
    global::shutdown_tracer_provider();
}

/// Helper macro to create spans with common attributes
#[macro_export]
macro_rules! span {
    ($name:expr) => {
        tracing::info_span!(
            $name,
            otel.kind = "internal",
            sigma.component = module_path!()
        )
    };
    ($name:expr, $($field:tt)*) => {
        tracing::info_span!(
            $name,
            otel.kind = "internal",
            sigma.component = module_path!(),
            $($field)*
        )
    };
}

/// Helper macro for instrumenting async functions
#[macro_export]
macro_rules! instrument_async {
    ($name:expr, $future:expr) => {
        async move {
            let span = $crate::span!($name);
            $future.instrument(span).await
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_config_default() {
        let config = TelemetryConfig::default();
        assert_eq!(config.service_name, "sigma-rs");
        assert_eq!(config.sampling_rate, 0.1);
        assert!(config.propagation);
    }

    #[test]
    fn test_telemetry_init_disabled() {
        // When telemetry feature is disabled, these should be no-ops
        assert!(init_telemetry(TelemetryConfig::default()).is_ok());
        assert!(init_telemetry_from_env().is_ok());
        shutdown_telemetry(); // Should not panic
    }
}
