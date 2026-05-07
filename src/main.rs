use anyhow::{Context, Result};
use opentelemetry::{
    KeyValue,
    global::{self},
};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{Resource, metrics::SdkMeterProvider};
use opentelemetry_semantic_conventions::{metric, resource};
use std::{
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

fn init_meter_provider() -> opentelemetry_sdk::metrics::SdkMeterProvider {
    let exporter = opentelemetry_otlp::MetricExporter::builder()
        .with_http()
        .with_endpoint("http://localhost:4318/v1/metrics")
        .build()
        .unwrap();

    let stdout_exporter = opentelemetry_stdout::MetricExporter::default();
    let provider = SdkMeterProvider::builder()
        .with_periodic_exporter(exporter)
        .with_periodic_exporter(stdout_exporter)
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
    provider
}

fn main() -> Result<()> {
    let shutdown = Arc::new(AtomicBool::new(false));

    let shutdown_handler = Arc::clone(&shutdown);
    ctrlc::set_handler(move || {
        shutdown_handler.store(true, std::sync::atomic::Ordering::Relaxed);
    })
    .context("failed to set signal handler")?;

    let meter_provider = init_meter_provider();

    let meter = global::meter("pgrpism");

    let start = Instant::now();

    let _uptime_counter = meter
        .f64_observable_gauge(metric::PROCESS_UPTIME)
        .with_description("Uptime of the process")
        .with_unit("s")
        .with_callback(move |observer| {
            observer.observe(start.elapsed().as_secs_f64(), &[]);
        })
        .build();

    let mut rt = monoio::RuntimeBuilder::<monoio::IoUringDriver>::new()
        .enable_timer()
        .build()
        .context("failed to build monoio runtime")?;

    rt.block_on(async move {
        println!("Running Ctrl-C to exit");
        while !shutdown.load(std::sync::atomic::Ordering::Relaxed) {
            monoio::time::sleep(Duration::from_millis(100)).await;
        }
        println!("Shutting down");
        meter_provider.shutdown()?;
        Ok(())
    })
}
