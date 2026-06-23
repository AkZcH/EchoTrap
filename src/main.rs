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

#[tokio::main]
async fn main() {
    init_tracing();

    let cfg = CliConfig::parse().merged();

    println!("2025-10-20T10:32:19.114652Z  INFO === EchoTrap Initialization Complete ===");
    println!("2025-10-20T10:32:19.114652Z  INFO Port: {}", cfg.port);
    println!("2025-10-20T10:32:19.114652Z  INFO Threshold: {}", cfg.threshold);
    println!("2025-10-20T10:32:19.114652Z  INFO Window: {}s", cfg.window);
    println!("2025-10-20T10:32:19.114652Z  INFO Log file: {}", cfg.log);

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    network::start_listener(cfg).await;
}