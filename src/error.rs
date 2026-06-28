// src/error.rs
//
// Structured error types for EchoTrap.
// All public-facing Results use these types instead of Box<dyn Error>.

use thiserror::Error;

/// Errors that can occur when starting or running the dashboard.
#[derive(Debug, Error)]
pub enum DashboardError {
    #[error("failed to bind dashboard on {addr}: {source}")]
    Bind {
        addr: String,
        #[source]
        source: std::io::Error,
    },

    #[error("dashboard server error: {0}")]
    Serve(#[from] std::io::Error),
}

/// Errors that can occur during port migration.
#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum MigrationError {
    #[error("no free port found after {attempts} attempts (excluding :{exclude})")]
    NoFreePort { attempts: usize, exclude: u16 },

    #[error("failed to bind new port :{port}: {source}")]
    BindFailed {
        port: u16,
        #[source]
        source: std::io::Error,
    },
}

/// Errors from socket option configuration.
#[derive(Debug, Error)]
pub enum SockoptError {
    #[error("failed to create socket: {0}")]
    Create(#[source] std::io::Error),

    #[error("failed to bind socket to {addr}: {source}")]
    Bind {
        addr: String,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to convert socket to async listener: {0}")]
    Convert(#[source] std::io::Error),
}
