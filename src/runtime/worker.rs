use std::sync::Arc;

use crate::config::Config;
use anyhow::{Context, Result};
use monoio::io::{AsyncReadRent, AsyncWriteRentExt};
use monoio::{
    self,
    net::{ListenerOpts, TcpListener, TcpStream},
};
use std::thread::JoinHandle;
use tokio_util::sync::CancellationToken;

pub fn run_workers(config: Arc<Config>, token: CancellationToken) -> Result<()> {
    let worker_count = (&config).general.worker_threads;
    let mut handles = Vec::with_capacity(worker_count);

    for worker_id in 0..worker_count {
        let worker_token = token.clone();
        let handle = std::thread::Builder::new()
            .name(format!("pgprism-worker-{worker_id}"))
            .spawn(move || {
                let mut rt = monoio::RuntimeBuilder::<monoio::IoUringDriver>::new()
                    .enable_timer()
                    .build()
                    .context("failed to build monoio runtime")?;

                rt.block_on(async move {
                    println!("Running worker {}", worker_id);

                    let listener_opts = ListenerOpts::new().reuse_port(true);

                    let listener = TcpListener::bind_with_config("0.0.0.0:50002", &listener_opts).context("Failed to bind")?;

                    loop {
                        monoio::select! {
                            _ = worker_token.cancelled() => {
                                println!("Shutting down worker {}", worker_id);
                                break;
                            }
                            incoming = listener.accept() => {
                                match incoming {
                                    Ok((stream, addr)) => {
                                        println!("accepted a connection on worker {} from: {}", worker_id, addr);
                                        monoio::spawn(echo(stream));
                                    }
                                    Err(e) => {
                                        println!("accepted connection failed: {}", e);
                                        return Err(e).context("failed to accept connection");
                                    }
                                }
                            }
                        }
                    }
                    Ok(())
                })
            })?;

        handles.push(handle);
    }

    join_workers(handles)?;

    Ok(())
}

fn join_workers(handles: Vec<JoinHandle<Result<()>>>) -> Result<()> {
    let mut had_failure = false;

    for (worker_id, handle) in handles.into_iter().enumerate() {
        match handle.join() {
            Ok(Ok(())) => {
                println!("worker {worker_id} exited cleanly");
            }
            Ok(Err(e)) => {
                eprintln!("worker {worker_id} returned error: {e:?}");
                had_failure = true;
            }
            Err(_) => {
                eprintln!("worker {worker_id} panicked");
                had_failure = true;
            }
        }
    }

    if had_failure {
        anyhow::bail!("one or more workers failed");
    }
    Ok(())
}

async fn echo(mut stream: TcpStream) -> Result<()> {
    let mut buf: Vec<u8> = Vec::with_capacity(8 * 1024);
    let mut res;
    loop {
        // read
        (res, buf) = stream.read(buf).await;
        if res? == 0 {
            return Ok(());
        }

        // write all
        (res, buf) = stream.write_all(buf).await;
        res?;

        // clear
        buf.clear();
    }
}
