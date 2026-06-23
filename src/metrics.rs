// src/metrics.rs
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Debug)]
pub struct Metrics {
    pub connection_count: AtomicUsize,
    pub attack_count: AtomicUsize,
    pub port_migrations: AtomicUsize,
    pub current_port: AtomicU64,
    start_secs: u64,
}

impl Metrics {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            connection_count: AtomicUsize::new(0),
            attack_count: AtomicUsize::new(0),
            port_migrations: AtomicUsize::new(0),
            current_port: AtomicU64::new(0),
            start_secs: unix_now(),
        })
    }

    pub fn inc_connections(&self) {
        self.connection_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_attacks(&self) {
        self.attack_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_migrations(&self) {
        self.port_migrations.fetch_add(1, Ordering::Relaxed);
    }

    pub fn set_port(&self, port: u16) {
        self.current_port.store(port as u64, Ordering::Relaxed);
    }

    pub fn uptime_secs(&self) -> u64 {
        unix_now().saturating_sub(self.start_secs)
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}