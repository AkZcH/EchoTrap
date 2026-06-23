// src/main.rs
mod config;
mod network;
mod detector;
mod migration;
mod logger;
mod metrics;
mod dashboard;
mod util;

use clap::Parser;
use config::CliConfig;
use logger::init_tracing;
use metrics::Metrics;
use tracing::info;

#[tokio::main]
async fn main() {
    init_tracing();

    let cfg = CliConfig::parse().merged();

    info!("=== EchoTrap Initialization Complete ===");
    info!("Port:      {}", cfg.port);
    info!("Threshold: {} hits", cfg.threshold);
    info!("Window:    {}s", cfg.window);
    info!("Log file:  {}", cfg.log);
    info!("Dashboard: 0.0.0.0:{}", cfg.dashboard_port);

    let metrics = Metrics::new();

    // Spawn dashboard on its own task — runs concurrently with the TCP listener.
    let dashboard_metrics = metrics.clone();
    let dashboard_port = cfg.dashboard_port;
    tokio::spawn(async move {
        if let Err(e) = dashboard::start_dashboard(dashboard_metrics, dashboard_port).await {
            tracing::error!("Dashboard error: {e}");
        }
    });

    network::start_listener(cfg, metrics).await;
}