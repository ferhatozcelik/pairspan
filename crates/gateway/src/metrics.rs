use std::fmt::Write;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

#[derive(Default)]
pub struct Metrics {
    pub connections_total: AtomicU64,
    pub auth_failures: AtomicU64,
    pub frames_relayed: AtomicU64,
    pub bytes_relayed: AtomicU64,
    pub frames_dropped: AtomicU64,
    pub slow_consumers: AtomicU64,
    pub rooms_opened: AtomicU64,
    pub rooms_joined: AtomicU64,
}

impl Metrics {
    /// Prometheus text exposition format.
    pub fn render(&self, connections: usize, rooms: usize) -> String {
        let mut out = String::with_capacity(1024);
        let mut line = |name: &str, kind: &str, value: u64| {
            let _ = writeln!(out, "# TYPE {name} {kind}\n{name} {value}");
        };
        line("pairspan_connections", "gauge", connections as u64);
        line("pairspan_rooms_open", "gauge", rooms as u64);
        line("pairspan_connections_total", "counter", self.connections_total.load(Relaxed));
        line("pairspan_auth_failures_total", "counter", self.auth_failures.load(Relaxed));
        line("pairspan_frames_relayed_total", "counter", self.frames_relayed.load(Relaxed));
        line("pairspan_bytes_relayed_total", "counter", self.bytes_relayed.load(Relaxed));
        line("pairspan_frames_dropped_total", "counter", self.frames_dropped.load(Relaxed));
        line("pairspan_slow_consumers_total", "counter", self.slow_consumers.load(Relaxed));
        line("pairspan_rooms_opened_total", "counter", self.rooms_opened.load(Relaxed));
        line("pairspan_rooms_joined_total", "counter", self.rooms_joined.load(Relaxed));
        out
    }
}
