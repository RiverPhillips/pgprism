use std::sync::Arc;
use std::time::Duration;

use crate::config::Config;
use anyhow::{Context, Result};
use monoio::io::{AsyncReadRent, AsyncWriteRent, AsyncWriteRentExt, Splitable};
use monoio::time;
use monoio::{
    self,
    net::{ListenerOpts, TcpListener, TcpStream},
};
use std::thread::JoinHandle;
use tokio_util::sync::CancellationToken;

pub fn run_workers(config: Arc<Config>, token: CancellationToken) -> Result<()> {
    let worker_count = (config).general.worker_threads;
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
                            _ = time::sleep(Duration::from_millis(10)) => {
                            }
                            _ = worker_token.cancelled() => {
                                println!("Shutting down worker {}", worker_id);
                                break;
                            }
                            downstream = listener.accept() => {
                                match downstream {
                                    Ok((downstream, addr)) => {
                                        println!("accepted a connection on worker {} from: {}", worker_id, addr);
                                        monoio::spawn(proxy(downstream));
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

async fn proxy(donwstream_conn: TcpStream) -> Result<()> {
    let upstream_conn = TcpStream::connect("127.0.0.1:5432").await;
    if let Ok(upstream_conn) = upstream_conn {
        monoio::spawn(async move {
            let (mut downstream_r, mut downstream_w) = donwstream_conn.into_split();
            let (mut upstream_r, mut upstream_w) = upstream_conn.into_split();
            let _ = monoio::join!(
                copy_one_direction(&mut upstream_r, &mut downstream_w),
                copy_one_direction(&mut downstream_r, &mut upstream_w),
            );
            println!("Finished copying")
        });
    } else {
        eprintln!("upstream dial failed")
    }
    Ok(())
}

async fn copy_one_direction<FROM: AsyncReadRent, TO: AsyncWriteRent>(
    mut from: FROM,
    to: &mut TO,
) -> Result<Vec<u8>, std::io::Error> {
    let mut buf = Vec::with_capacity(8 * 1024);
    let mut res;
    loop {
        // read
        (res, buf) = from.read(buf).await;
        if res? == 0 {
            return Ok(buf);
        }

        // write all
        (res, buf) = to.write_all(buf).await;
        res?;

        // clear
        buf.clear();
    }
}
