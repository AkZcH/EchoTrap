// src/dashboard.rs
// Full implementation in C-13 (Prometheus + Axum dashboard commit).
#![allow(dead_code)]

use crate::metrics::Metrics;
use std::sync::Arc;

pub async fn start_dashboard(_metrics: Arc<Metrics>, _port: u16) -> Result<(), Box<dyn std::error::Error>> {
    // TODO C-13: serve /metrics and /status via Axum
    Ok(())
}