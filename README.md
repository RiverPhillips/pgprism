# pgprism

A high-performance Postgres proxy that turns traffic into insight.

pgprism sits between your application and Postgres, transparently forwarding the wire protocol while emitting rich OpenTelemetry metrics about every connection and query. Like a prism, it takes one stream of traffic and decomposes it into the signals you need to understand and operate your database.

## Status

**Pre-alpha. Active development on the MVP.**

## MVP Scope

The MVP is deliberately narrow: **observability for a single Postgres instance, with minimal proxy overhead**. No pooling, no routing, no failover, no TLS termination. Just a transparent forwarder that emits excellent telemetry.

Concretely, the MVP delivers:

- A TCP proxy that accepts client connections, dials a configured upstream Postgres, and forwards bytes in both directions.
- Per-connection lifecycle metrics: counts of connections accepted, established, terminated, and currently active, broken down by termination reason.
- Per-query metrics: query rate, latency distribution (as an exponential histogram for accurate cross-instance percentiles), and error rates with SQLSTATE codes.
- OpenTelemetry-native instrumentation, exported via OTLP, with sensible defaults that follow OTel semantic conventions.
- Static TOML configuration for the listen address and a single upstream URL.

This is the smallest version of pgprism that is genuinely useful. It deploys as a sidecar in front of any Postgres database and gives you visibility you can't get from `pg_stat_statements` alone — live latency distributions, real-time error streams, and per-connection lifetimes — without any database-side configuration.

## Design Principles

These principles guide every design decision and will shape pgprism well beyond the MVP.

### Goodput over throughput

pgprism is designed to maximise the rate of *successful, timely* requests, not the raw rate of accepted requests. Under load, it will reject work rather than degrade service for accepted work. Bounded queues, fixed concurrency limits, and fast rejection are first-class concerns. A request that has to wait longer than it would have taken to serve is wasted work, and queueing it further only makes the problem worse.

### Static stability

pgprism's behaviour under load is the same as its behaviour at rest, just with more rejection. There are no mode switches, no fallback logic, no "emergency mode" that activates under stress. Mode-switching code paths are exercised rarely, are hard to test, and tend to fail precisely when they are needed most. pgprism fails the same way it succeeds.

### Observability is not in the data path

pgprism continues to proxy queries even when its observability backend is unreachable. Metrics are emitted best-effort; failures to export do not block, retry indefinitely, or interfere with proxying. Your database stays available even when your observability stack does not.

### Minimal overhead

Bytes pass through pgprism with as little processing as the protocol allows. Metric recording is in-memory atomic operations on the hot path; export happens in the background. The proxy is built on monoio and io_uring for thread-per-core efficiency, with a clear path to eBPF acceleration of the steady-state forwarding path in future versions.

## Naming Conventions

pgprism follows Envoy's terminology for proxy-side concepts:

- **Downstream** is the client-facing side: the application connecting to pgprism.
- **Upstream** is the backend-facing side: the Postgres instance pgprism is connecting to.

This convention applies consistently across metric names, log fields, configuration keys, and code. `downstream.connections.active` is the count of clients currently connected to pgprism; `upstream.connections.active` is the count of connections pgprism currently holds open to Postgres.

## Quick Start

```bash
git clone https://github.com/yourname/pgprism
cd pgprism
docker compose up -d
cargo run --release
```

Point your Postgres client at `localhost:6432` instead of `localhost:5432` and pgprism will transparently forward to the configured upstream. Open Grafana at `localhost:3000` to see metrics flowing.

## Configuration

pgprism is configured via a single TOML file, defaulting to `pgprism.toml` in the working directory.

```toml
[general]
worker_threads = 4

[downstream]
listen_address = "0.0.0.0:6432"

[upstream]
url = "postgresql://user:password@localhost:5432/dbname"

[telemetry]
otlp_endpoint = "http://localhost:4318"
service_name = "pgprism"
```

Configuration is loaded once at startup. Reloading requires a restart.

## Architecture

pgprism is built on:

- **monoio** for thread-per-core async I/O on io_uring
- **OpenTelemetry** for metrics emission (OTLP exporter)
- **Rust** with strict lint settings, no `unsafe` outside of well-justified hot-path regions

Each accepted downstream connection is paired with an upstream connection for its lifetime. Bytes are forwarded with minimal protocol parsing — only enough to identify connection lifecycle events, query boundaries, and error responses. Per-connection metrics are recorded into atomic counters, aggregated at OTel export time.

## Roadmap

The MVP intentionally excludes features that are common in production Postgres proxies. They are planned for subsequent versions:

- **v1**: Connection pooling (transaction-mode), with bounded queues and per-upstream concurrency limits.
- **v2**: Read/write splitting across a primary and replicas, with transaction-aware routing.
- **v3**: Health-aware routing, per-replica lag tracking, and operator-facing admission control.
- **v4**: eBPF-accelerated forwarding for steady-state traffic, keeping protocol logic in userspace and the byte path in the kernel.

Each version is a complete, deployable, useful tool. There is no "v1 will be useful, I promise" middle phase.

## Non-Goals

pgprism is not, and will not be:

- A general-purpose L4 load balancer (use HAProxy or Envoy).
- A SQL-aware firewall or query rewriter.
- A drop-in replacement for `pg_stat_statements` or other database-side instrumentation. pgprism complements them; it does not replace them.
- A multi-tenant or multi-database aware proxy. Each pgprism instance fronts one logical Postgres deployment.
