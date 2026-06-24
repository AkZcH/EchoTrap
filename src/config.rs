// src/config.rs
use clap::Parser;
use serde::Deserialize;
use std::fmt;

// ── Validation error ──────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct ConfigError(String);

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ConfigError {}

// ── CLI config ────────────────────────────────────────────────────────────────

#[derive(Debug, Parser, Clone)]
#[command(name = "EchoTrap", about = "A self-rebuilding TCP honeypot")]
pub struct CliConfig {
    /// Starting TCP port (1024–32767 or 61000–65535)
    #[arg(short, long, default_value_t = 9000)]
    pub port: u16,

    /// Number of hits from one IP before migration (1–1000)
    #[arg(short, long, default_value_t = 5)]
    pub threshold: u32,

    /// Time window for detection in seconds (1–3600)
    #[arg(short, long, default_value_t = 10)]
    pub window: u64,

    /// Path to log file (parent directory must be writable)
    #[arg(long, default_value = "./echotrap.log")]
    pub log: String,

    /// Port for the HTTP dashboard/metrics server
    #[arg(long, default_value_t = 8081)]
    pub dashboard_port: u16,

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
    pub dashboard_port: Option<u16>,
}

impl CliConfig {
    pub fn merged(self) -> Self {
        if let Some(cfg_path) = &self.config {
            match std::fs::read_to_string(cfg_path) {
                Err(e) => {
                    crate::display::warn(&format!(
                        "Could not read config file {cfg_path}: {e} — using CLI defaults"
                    ));
                }
                Ok(data) => match toml::from_str::<FileConfig>(&data) {
                    Err(e) => {
                        crate::display::warn(&format!(
                            "Could not parse config file {cfg_path}: {e} — using CLI defaults"
                        ));
                    }
                    Ok(file_cfg) => {
                        return CliConfig {
                            port: file_cfg.port.unwrap_or(self.port),
                            threshold: file_cfg.threshold.unwrap_or(self.threshold),
                            window: file_cfg.window.unwrap_or(self.window),
                            log: file_cfg.log.unwrap_or(self.log),
                            dashboard_port: file_cfg
                                .dashboard_port
                                .unwrap_or(self.dashboard_port),
                            config: self.config,
                        };
                    }
                },
            }
        }
        self
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        let mut errors: Vec<String> = Vec::new();

        validate_port(self.port, "port", &mut errors);
        validate_port(self.dashboard_port, "dashboard-port", &mut errors);

        if self.port == self.dashboard_port {
            errors.push(format!(
                "port ({}) and dashboard-port ({}) must be different",
                self.port, self.dashboard_port
            ));
        }

        if self.threshold == 0 {
            errors.push("threshold must be at least 1".into());
        }
        if self.threshold > 1000 {
            errors.push(format!(
                "threshold {} is unreasonably large (max 1000)",
                self.threshold
            ));
        }

        if self.window == 0 {
            errors.push("window must be at least 1 second".into());
        }
        if self.window > 3600 {
            errors.push(format!(
                "window {}s exceeds maximum of 3600s (1 hour)",
                self.window
            ));
        }

        let log_path = std::path::Path::new(&self.log);
        let parent = log_path.parent().unwrap_or(std::path::Path::new("."));
        if !parent.exists() {
            errors.push(format!(
                "log path parent directory '{}' does not exist",
                parent.display()
            ));
        } else if let Err(e) = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(log_path)
        {
            errors.push(format!("log path '{}' is not writable: {e}", self.log));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(ConfigError(errors.join("\n")))
        }
    }
}

fn validate_port(port: u16, name: &str, errors: &mut Vec<String>) {
    if port < 1024 {
        errors.push(format!(
            "{name} {port} is in the privileged range (<1024) — choose a port ≥1024"
        ));
    } else if (32768..=60999).contains(&port) {
        errors.push(format!(
            "{name} {port} is in the Linux ephemeral range (32768–60999) — \
             choose a port outside this range"
        ));
    }
}