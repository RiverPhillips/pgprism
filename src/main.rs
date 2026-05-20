use anyhow::{Context, Result};
use clap::Parser;
use opentelemetry::global;
use pgprism::config::Config;
use pgprism::observability::metrics::Metrics;
use pgprism::{observability, runtime};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value_t = String::from("pgprism.toml"), env)]
    pub config_file: String,
}

fn main() -> Result<()> {
    let start_time = Instant::now();
    let token = CancellationToken::new();
    let token_for_handlers = token.clone();
    ctrlc::set_handler(move || {
        token.cancel();
    })
    .context("failed to set signal handler")?;

    let args = Args::parse();

    let config_path = Path::new(&args.config_file);
    let config = Arc::new(Config::load(config_path)?);

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
