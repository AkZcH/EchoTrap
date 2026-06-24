// src/main.rs
mod config;
mod network;
mod detector;
mod migration;
mod logger;
mod metrics;
mod dashboard;
mod display;
mod util;

use clap::Parser;
use config::CliConfig;
use logger::init_tracing;
use metrics::Metrics;

#[tokio::main]
async fn main() {
    init_tracing();

    let cfg = CliConfig::parse().merged();

    // Validate before printing the header — on error, show problems and exit.
    if let Err(e) = cfg.validate() {
        display::error("Configuration errors:");
        for line in e.to_string().lines() {
            display::error(&format!("  {line}"));
        }
        std::process::exit(1);
    }

    display::print_header(env!("CARGO_PKG_VERSION"));
    display::print_field("port",      &cfg.port.to_string());
    display::print_field("threshold", &format!("{} hits", cfg.threshold));
    display::print_field("window",    &format!("{}s", cfg.window));
    display::print_field("log",       &cfg.log);
    display::print_field("dashboard", &format!("0.0.0.0:{}", cfg.dashboard_port));
    display::separator();

    let metrics = Metrics::new();

    let dashboard_metrics = metrics.clone();
    let dashboard_port = cfg.dashboard_port;
    tokio::spawn(async move {
        if let Err(e) = dashboard::start_dashboard(dashboard_metrics, dashboard_port).await {
            display::error(&format!("Dashboard error: {e}"));
        }
    });

    network::start_listener(cfg, metrics).await;
}