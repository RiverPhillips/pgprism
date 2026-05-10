use std::time::{Duration, Instant};

use anyhow::Result;
use opentelemetry::{KeyValue, global};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{Resource, metrics::SdkMeterProvider};
use opentelemetry_semantic_conventions::{metric, resource};

pub struct Provider {
    meter_provider: opentelemetry_sdk::metrics::SdkMeterProvider,
}

impl Provider {
    pub fn new() -> Result<Self> {
        let meter_provider = build_meter_provider()?;
        register_process_metrics();
        Ok(Self { meter_provider })
    }

    pub fn shutdown(self) -> Result<()> {
        self.meter_provider
            .shutdown_with_timeout(Duration::from_secs(1))?;
        Ok(())
    }
}

fn build_meter_provider() -> Result<opentelemetry_sdk::metrics::SdkMeterProvider> {
    let exporter = opentelemetry_otlp::MetricExporter::builder()
        .with_http()
        .with_endpoint("http://localhost:4318/v1/metrics")
        .build()?;

    let provider = SdkMeterProvider::builder()
        .with_periodic_exporter(exporter)
        .with_resource(
            Resource::builder()
                .with_service_name("pgprism")
                .with_attribute(KeyValue::new("service.version", env!("CARGO_PKG_VERSION")))
                .with_attribute(KeyValue::new(
                    resource::PROCESS_PID,
                    i64::from(std::process::id()),
                ))
                .with_attribute(KeyValue::new(
                    resource::SERVICE_INSTANCE_ID,
                    uuid::Uuid::new_v4().to_string(),
                ))
                .build(),
        )
        .build();
    global::set_meter_provider(provider.clone());
    Ok(provider)
}

fn register_process_metrics() {
    let meter = global::meter("pgprism");
    let start = Instant::now();

    let _uptime_counter = meter
        .f64_observable_gauge(metric::PROCESS_UPTIME)
        .with_description("Uptime of the process")
        .with_unit("s")
        .with_callback(move |observer| {
            observer.observe(start.elapsed().as_secs_f64(), &[]);
        })
        .build();
}
