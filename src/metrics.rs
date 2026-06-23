// src/metrics.rs
// Wired into network.rs and dashboard.rs in C-13.
#![allow(dead_code)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Metrics {
    pub connection_count: AtomicUsize,
    pub attack_count: AtomicUsize,
    pub port_migrations: AtomicUsize,
}

impl Metrics {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            connection_count: AtomicUsize::new(0),
            attack_count: AtomicUsize::new(0),
            port_migrations: AtomicUsize::new(0),
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
}