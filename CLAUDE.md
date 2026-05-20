# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build                  # build
cargo clippy                 # lint
cargo test                   # run all tests
cargo test <test_name>       # run a single test
cargo run                    # run the proxy (requires io_uring / Linux)
```

Clippy lints are strict and enforced — treat all warnings as real issues. The lint profile bans panics, unwraps, silent error drops, and unsafe code without justification.

Tests are a required part of every change. Unit tests live in `#[cfg(test)]` modules in the same file as the code they test.

## Architecture

pgprism is a **thread-per-core, shared-nothing Postgres proxy** built on [monoio](https://github.com/bytedance/monoio) (io_uring-backed async runtime).

**Execution model:** `main` spawns one OS thread per CPU core (`config.general.worker_threads`). Each thread runs an independent monoio runtime with its own io_uring instance. Workers never coordinate on the hot path.

**Connection distribution:** Each worker binds its own `TcpListener` on the same port with `SO_REUSEPORT`. The kernel hashes incoming connections across workers. A connection stays on the worker that accepted it for its entire lifetime — no work stealing, no cross-worker communication for per-connection state.

**Proxy path:** downstream client → pgprism listener (address/port from `DownstreamConfig`) → upstream Postgres (`127.0.0.1:5432`). Currently a raw TCP splice (`copy_one_direction`). The Postgres wire protocol is not yet parsed.

**Cross-worker coordination (explicit seams only):**
- `Arc<Config>` — cloned at startup, immutable for process lifetime (TOML-parsed, static config)
- `Arc<Metrics>` — OpenTelemetry counters/gauges backed by atomics; safe to read from any thread
- `CancellationToken` — shutdown signal propagated from `ctrlc` handler to all workers

**Observability:** OTLP HTTP export to `http://localhost:4318/v1/metrics`. Metrics are defined in `src/observability/metrics.rs` using OTel semantic conventions. The `Provider` in `src/observability/mod.rs` owns the `SdkMeterProvider` and must be shut down after workers exit. Shutdown flushes pending metrics to the collector — if the collector isn't running this fails; treat it as non-fatal (log, don't propagate). The full local stack (via `docker compose up`) is: pgprism → OTel Collector (`:4318`) → ClickHouse (`otel` db) → Grafana (`:3000`). Dashboards are provisioned as code in `.config/grafana/provisioning/dashboards/` — don't clickops, export via `GET /api/dashboards/uid/<uid>` and update `pgprism.json`.

`UpDownCounter` values must be cloned before being moved into a `monoio::spawn` closure — clone before the spawn, then use the clone inside the task to record the decrement.

`monoio::time::Instant` is only valid inside an active monoio runtime — use `std::time::Instant` for timestamps captured in `main()` before workers are spawned (e.g. process start time for the uptime gauge).

Each accepted connection is already running in its own spawned task (from the accept loop). `proxy()` should `await` the bidirectional copy directly rather than spawning an inner task — an inner spawn causes `proxy()` to return immediately, which fires any post-proxy cleanup (metric decrements, resource drops) before the connection actually closes.

Config struct fields should use semantic types rather than `String` — e.g. `IpAddr` for addresses. Serde deserializes these natively from TOML strings, and `const` values (e.g. `IpAddr::V4(Ipv4Addr::new(...))`) can serve as defaults without any runtime parsing or `.unwrap()`.

**Module layout:**
- `src/config.rs` — `Config` / `General` / `DownstreamConfig` structs, TOML-deserializable
- `src/error.rs` — `Error` enum, thiserror-backed; covers config I/O and parse errors
- `src/runtime/worker.rs` — `Worker`, `run_workers`, `proxy`, `copy_one_direction`
- `src/observability/mod.rs` — OTel provider setup and shutdown
- `src/observability/metrics.rs` — all instrument definitions

## Terminology

- **Downstream** — a client connecting *to* pgprism
- **Upstream** — a Postgres instance that pgprism connects *to*

## Design constraints

- Queues must be bounded; bounded rejection is preferred over unbounded growth
- No SQL parsing or rewriting (non-goal)
- One pgprism instance per logical Postgres deployment (no multi-tenancy)
- Config is static — no live reload; changes require a restart
