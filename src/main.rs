use anyhow::{Context, Result};
use pgprism::config::Config;
use pgprism::{observability, runtime};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

fn main() -> Result<()> {
    let token = CancellationToken::new();
    let token_for_handlers = token.clone();
    ctrlc::set_handler(move || {
        token.cancel();
    })
    .context("failed to set signal handler")?;
    let config = Arc::new(Config::default());

    runtime::run_workers(config, token_for_handlers)?;

    let observability_provider = observability::Provider::new()?;

    println!("Shutting down...");

    observability_provider.shutdown()?;

    Ok(())
}
