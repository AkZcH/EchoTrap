use clap::Parser;
use serde::Deserialize;

/// Configuration loaded either from CLI or file
#[derive(Debug, Parser, Clone)]
#[command(name = "EchoTrap", about = "A self-rebuilding TCP honeypot")]
pub struct CliConfig {
    /// Starting TCP port
    #[arg(short, long, default_value_t = 8080)]
    pub port: u16,

    /// Number of hits from one IP before migration
    #[arg(short, long, default_value_t = 5)]
    pub threshold: u32,

    /// Time window for detection (seconds)
    #[arg(short, long, default_value_t = 10)]
    pub window: u64,

    /// Optional path to log file
    #[arg(long, default_value = "./echotrap.log")]
    pub log: String,

    /// Optional config file (TOML)
    #[arg(long)]
    pub config: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FileConfig {
    pub port: Option<u16>,
    pub threshold: Option<u32>,
    pub window: Option<u64>,
    pub log: Option<String>,
}

impl CliConfig {
    /// Merge CLI + optional config file values
    pub fn merged(self) -> Self {
        if let Some(cfg_path) = &self.config {
            if let Ok(data) = std::fs::read_to_string(cfg_path) {
                if let Ok(file_cfg) = toml::from_str::<FileConfig>(&data) {
                    return CliConfig {
                        port: file_cfg.port.unwrap_or(self.port),
                        threshold: file_cfg.threshold.unwrap_or(self.threshold),
                        window: file_cfg.window.unwrap_or(self.window),
                        log: file_cfg.log.unwrap_or(self.log),
                        config: self.config,
                    };
                }
            }
        }
        self
    }
}