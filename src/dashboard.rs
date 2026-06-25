// src/dashboard.rs
use crate::metrics::Metrics;
use axum::{extract::State, http::StatusCode, response::Json, routing::get, Router};
use serde::Serialize;
use std::sync::Arc;
use tracing::info;

#[derive(Serialize)]
struct StatusResponse {
    version: &'static str,
    current_port: u16,
    uptime_secs: u64,
}

#[derive(Serialize)]
struct MetricsResponse {
    connections_total: usize,
    attacks_detected: usize,
    port_migrations: usize,
    current_port: u16,
    uptime_secs: u64,
}

async fn handle_status(State(metrics): State<Arc<Metrics>>) -> Json<StatusResponse> {
    Json(StatusResponse {
        version: env!("CARGO_PKG_VERSION"),
        current_port: metrics
            .current_port
            .load(std::sync::atomic::Ordering::Relaxed) as u16,
        uptime_secs: metrics.uptime_secs(),
    })
}

async fn handle_metrics(State(metrics): State<Arc<Metrics>>) -> Json<MetricsResponse> {
    Json(MetricsResponse {
        connections_total: metrics
            .connection_count
            .load(std::sync::atomic::Ordering::Relaxed),
        attacks_detected: metrics
            .attack_count
            .load(std::sync::atomic::Ordering::Relaxed),
        port_migrations: metrics
            .port_migrations
            .load(std::sync::atomic::Ordering::Relaxed),
        current_port: metrics
            .current_port
            .load(std::sync::atomic::Ordering::Relaxed) as u16,
        uptime_secs: metrics.uptime_secs(),
    })
}

async fn handle_health() -> StatusCode {
    StatusCode::OK
}

pub async fn start_dashboard(
    metrics: Arc<Metrics>,
    port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    let app = Router::new()
        .route("/health", get(handle_health))
        .route("/status", get(handle_status))
        .route("/metrics", get(handle_metrics))
        .with_state(metrics);

    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Dashboard listening on http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}
