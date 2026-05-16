use anyhow::{Context, Result};
use opentelemetry::global;
use std::time::Instant;
use pgprism::config::Config;
use pgprism::observability::metrics::Metrics;
use pgprism::{observability, runtime};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

fn main() -> Result<()> {
    let start_time = Instant::now();
    let token = CancellationToken::new();
    let token_for_handlers = token.clone();
    ctrlc::set_handler(move || {
        token.cancel();
    })
    .context("failed to set signal handler")?;
    let config = Arc::new(Config::default());

    let observability_provider = observability::Provider::new()?;
    let meter = global::meter("pgprism");
    let metrics = Arc::new(Metrics::new(meter, start_time));

    runtime::run_workers(config, token_for_handlers, metrics)?;

    println!("Shutting down...");

    if let Err(e) = observability_provider.shutdown() {
        eprintln!("metrics flush on shutdown failed: {e}");
    }

    Ok(())
}
