use std::sync::Arc;
use axum::{Router, routing::get, Json};
use serde_json::json;
use crate::metrics::Metrics;

pub async fn start_dashboard(metrics: Arc<Metrics>, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let app = Router::new().route("/metrics", get({
        let m = metrics.clone();
        move || async move {
            Json(json!({
                "connections": m.connection_count.load(std::sync::atomic::Ordering::Relaxed),
                "attacks": m.attack_count.load(std::sync::atomic::Ordering::Relaxed),
                "migrations": m.port_migrations.load(std::sync::atomic::Ordering::Relaxed),
            }))
        }
    }));

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    tracing::info!("Metrics dashboard running at http://127.0.0.1:{}/metrics", port);
    axum::serve(listener, app).await?;
    Ok(())
}