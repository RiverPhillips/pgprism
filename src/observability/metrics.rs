use std::time::Instant;
use opentelemetry::metrics::{Counter, Meter, ObservableGauge, UpDownCounter};
use opentelemetry_semantic_conventions::metric;

pub struct Metrics {
    pub downstream_connections_accepted: Counter<u64>,
    pub downstream_connections_active: UpDownCounter<i64>,

    pub upstream_connections_established: Counter<u64>,
    pub upstream_connections_active: UpDownCounter<i64>,

    _uptime_counter: ObservableGauge<f64>,
}

impl Metrics {
    pub fn new(meter: Meter, start_time: Instant) -> Self {
        let uptime_counter = meter
            .f64_observable_gauge(metric::PROCESS_UPTIME)
            .with_description("Uptime of the process")
            .with_unit("s")
            .with_callback(move |observer| {
                observer.observe(start_time.elapsed().as_secs_f64(), &[]);
            })
            .build();

        Self {
            _uptime_counter: uptime_counter,
            downstream_connections_accepted: meter
                .u64_counter("pgprism.downstream_connections.accepted")
                .with_description("Total number of downstreams connections accepted")
                .build(),
            downstream_connections_active: meter
                .i64_up_down_counter("pgprism.downstream_connections.active")
                .with_description("Currently established downstream connections")
                .build(),
            upstream_connections_established: meter
                .u64_counter("pgprism.upstream_connections.established")
                .with_description("Total number of upstream connections established")
                .build(),
            upstream_connections_active: meter
                .i64_up_down_counter("pgprism.upstream_connections.active")
                .with_description("Currently established upstream connections")
                .build(),
        }
    }
}
