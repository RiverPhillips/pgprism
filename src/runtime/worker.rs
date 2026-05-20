use std::sync::Arc;
use std::time::Duration;

use crate::config::Config;
use crate::observability::metrics::Metrics;
use anyhow::{Context, Result};
use monoio::io::{AsyncReadRent, AsyncWriteRent, AsyncWriteRentExt, Splitable};
use monoio::time;
use monoio::{
    self,
    net::{ListenerOpts, TcpListener, TcpStream},
};
use std::thread::JoinHandle;
use tokio_util::sync::CancellationToken;

struct Worker {
    metrics: Arc<Metrics>,
    config: Arc<Config>,
    id: usize,
}

impl Worker {
    pub fn new(metrics: Arc<Metrics>, config: Arc<Config>, id: usize) -> Self {
        Self {
            metrics,
            id,
            config,
        }
    }

    pub fn run(self, worker_token: CancellationToken) -> Result<()> {
        let mut rt = monoio::RuntimeBuilder::<monoio::IoUringDriver>::new()
            .enable_timer()
            .build()
            .context("failed to build monoio runtime")?;

        rt.block_on(async move {
                    println!("Running worker {}", self.id);

                    let listener_opts = ListenerOpts::new().reuse_port(true);

                    let listener = TcpListener::bind_with_config(
                        (self.config.downstream.listener_address, self.config.downstream.listener_port),
                        &listener_opts,
                    )
                    .context("Failed to bind")?;

                    loop {
                        monoio::select! {
                            _ = time::sleep(Duration::from_millis(10)) => {
                            }
                            _ = worker_token.cancelled() => {
                                println!("Shutting down worker {}", self.id);
                                break;
                            }
                            downstream = listener.accept() => {
                                match downstream {
                                    Ok((downstream, addr)) => {
                                        self.metrics.downstream_connections_accepted.add(1, &[]);
                                        self.metrics.downstream_connections_active.add(1, &[]);
                                        println!("accepted a connection on worker {} from: {}", self.id, addr);
                                        let metrics = self.metrics.clone();
                                        monoio::spawn(async move {
                                            if let Err(e) = proxy(downstream, metrics).await {
                                                eprintln!("proxy error: {e}");
                                            }
                                        });
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
                })?;
        Ok(())
    }
}

async fn proxy(downstream_conn: TcpStream, metrics: Arc<Metrics>) -> Result<()> {
    let upstream_conn = TcpStream::connect("127.0.0.1:5432").await;
    if let Ok(upstream_conn) = upstream_conn {
        metrics.upstream_connections_established.add(1, &[]);
        metrics.upstream_connections_active.add(1, &[]);
        let (mut downstream_r, mut downstream_w) = downstream_conn.into_split();
        let (mut upstream_r, mut upstream_w) = upstream_conn.into_split();
        drop(monoio::join!(
            copy_one_direction(&mut upstream_r, &mut downstream_w),
            copy_one_direction(&mut downstream_r, &mut upstream_w),
        ));
        metrics.upstream_connections_active.add(-1, &[]);
    } else {
        eprintln!("upstream dial failed");
    }
    metrics.downstream_connections_active.add(-1, &[]);
    Ok(())
}

pub fn run_workers(
    config: Arc<Config>,
    token: CancellationToken,
    metrics: Arc<Metrics>,
) -> Result<()> {
    let worker_count = config.general.worker_threads;
    let mut handles = Vec::with_capacity(worker_count);

    for id in 0..worker_count {
        let worker = Worker::new(metrics.clone(), config.clone(), id);
        let worker_token = token.clone();
        let handle = std::thread::Builder::new()
            .name(format!("pgprism-worker-{id}"))
            .spawn(|| worker.run(worker_token))?;
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

async fn copy_one_direction<FROM: AsyncReadRent, TO: AsyncWriteRent>(
    mut from: FROM,
    to: &mut TO,
) -> Result<Vec<u8>, std::io::Error> {
    let mut buf = Vec::with_capacity(8 * 1024);
    let mut res;
    loop {
        (res, buf) = from.read(buf).await;
        if res? == 0 {
            return Ok(buf);
        }

        (res, buf) = to.write_all(buf).await;
        res?;

        buf.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_workers_all_succeed() {
        let handles = (0..2)
            .map(|_| std::thread::spawn(|| -> Result<()> { Ok(()) }))
            .collect();
        assert!(join_workers(handles).is_ok());
    }

    #[test]
    fn join_workers_one_fails() {
        let handles = vec![
            std::thread::spawn(|| -> Result<()> { Ok(()) }),
            std::thread::spawn(|| -> Result<()> { anyhow::bail!("boom") }),
        ];
        assert!(join_workers(handles).is_err());
    }

    #[test]
    fn join_workers_one_panics() {
        let handles = vec![
            std::thread::spawn(|| -> Result<()> { Ok(()) }),
            std::thread::spawn(|| -> Result<()> { panic!("oops") }),
        ];
        assert!(join_workers(handles).is_err());
    }
}
