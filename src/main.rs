// src/main.rs
mod config;
mod dashboard;
mod detector;
mod display;
mod error;
mod logger;
mod metrics;
mod migration;
mod network;
mod persona;
mod personas;
mod redirect;
mod sockopt;
mod util;

use clap::Parser;
use config::CliConfig;
use metrics::Metrics;

#[tokio::main]
async fn main() {
    let cfg = CliConfig::parse().merged();

    if let Err(e) = cfg.validate() {
        display::error("Configuration errors:");
        for line in e.to_string().lines() {
            display::error(&format!("  {line}"));
        }
        std::process::exit(1);
    }

    // Init tracing after validation so we know the log path is writable.
    // _guard must stay alive for the process lifetime — dropping it flushes
    // the non-blocking JSON file writer.
    let _guard = logger::init_tracing(&cfg.log);

    display::print_header(env!("CARGO_PKG_VERSION"));
    display::print_field("port", &cfg.port.to_string());
    display::print_field("threshold", &format!("{} hits", cfg.threshold));
    display::print_field("window", &format!("{}s", cfg.window));
    display::print_field("persona", &cfg.persona.to_string());
    display::print_field("log", &cfg.log);
    display::print_field("max-conn", &cfg.max_connections.to_string());
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
