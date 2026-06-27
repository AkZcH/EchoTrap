// src/dashboard.rs
use crate::error::DashboardError;
use crate::metrics::Metrics;
use axum::{
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
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

async fn handle_health() -> StatusCode {
    StatusCode::OK
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

async fn handle_metrics_json(State(metrics): State<Arc<Metrics>>) -> Json<MetricsResponse> {
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

async fn handle_metrics_prometheus(State(metrics): State<Arc<Metrics>>) -> impl IntoResponse {
    let connections = metrics
        .connection_count
        .load(std::sync::atomic::Ordering::Relaxed);
    let attacks = metrics
        .attack_count
        .load(std::sync::atomic::Ordering::Relaxed);
    let migrations = metrics
        .port_migrations
        .load(std::sync::atomic::Ordering::Relaxed);
    let port = metrics
        .current_port
        .load(std::sync::atomic::Ordering::Relaxed);
    let uptime = metrics.uptime_secs();
    let version = env!("CARGO_PKG_VERSION");

    let body = format!(
        "# HELP echotrap_connections_total Total TCP connections accepted\n\
         # TYPE echotrap_connections_total counter\n\
         echotrap_connections_total {connections}\n\
         \n\
         # HELP echotrap_attacks_detected_total Scan/attack events detected\n\
         # TYPE echotrap_attacks_detected_total counter\n\
         echotrap_attacks_detected_total {attacks}\n\
         \n\
         # HELP echotrap_port_migrations_total Port migrations triggered\n\
         # TYPE echotrap_port_migrations_total counter\n\
         echotrap_port_migrations_total {migrations}\n\
         \n\
         # HELP echotrap_current_port Currently active honeypot port\n\
         # TYPE echotrap_current_port gauge\n\
         echotrap_current_port {port}\n\
         \n\
         # HELP echotrap_uptime_seconds Process uptime in seconds\n\
         # TYPE echotrap_uptime_seconds counter\n\
         echotrap_uptime_seconds {uptime}\n\
         \n\
         # HELP echotrap_info Static build information\n\
         # TYPE echotrap_info gauge\n\
         echotrap_info{{version=\"{version}\"}} 1\n\
         "
    );

    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; version=0.0.4; charset=utf-8"),
    );

    (headers, body)
}

/// Start the dashboard HTTP server.
/// Returns a structured `DashboardError` instead of `Box<dyn Error>`.
pub async fn start_dashboard(metrics: Arc<Metrics>, port: u16) -> Result<(), DashboardError> {
    let app = Router::new()
        .route("/health", get(handle_health))
        .route("/status", get(handle_status))
        .route("/metrics", get(handle_metrics_json))
        .route("/metrics/prometheus", get(handle_metrics_prometheus))
        .with_state(metrics);

    let addr = format!("0.0.0.0:{port}");
    let listener =
        tokio::net::TcpListener::bind(&addr)
            .await
            .map_err(|e| DashboardError::Bind {
                addr: addr.clone(),
                source: e,
            })?;

    info!("Dashboard listening on http://{addr}");
    axum::serve(listener, app)
        .await
        .map_err(DashboardError::Serve)?;

    Ok(())
}
