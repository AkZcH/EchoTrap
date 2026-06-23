// src/logger.rs
use tracing_subscriber::EnvFilter;

pub fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .with_level(true)
        .with_line_number(true)
        .compact()
        .init();
}