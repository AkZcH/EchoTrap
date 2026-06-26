//! EchoTrap test harness — exposes internals for integration tests.
//!
//! This lib target is only used by `tests/integration.rs`.
//! The binary entry point remains `src/main.rs`.

pub mod config;
pub mod dashboard;
pub mod detector;
pub mod display;
pub mod logger;
pub mod metrics;
pub mod migration;
pub mod network;
pub mod persona;
pub mod personas;
pub mod sockopt;
pub mod util;

use metrics::Metrics;
use persona::Persona;
use tokio::task::JoinHandle;

/// Persona alias for test use — mirrors persona::Persona.
pub enum TestPersona {
    Ssh,
    Http,
    Redis,
    Raw,
}

impl From<TestPersona> for Persona {
    fn from(p: TestPersona) -> Self {
        match p {
            TestPersona::Ssh => Persona::Ssh,
            TestPersona::Http => Persona::Http,
            TestPersona::Redis => Persona::Redis,
            TestPersona::Raw => Persona::Raw,
        }
    }
}

/// Spawn a single persona listener on the given port for test use.
/// Returns a JoinHandle — drop it to stop caring about the task,
/// or abort() it explicitly to shut the listener down.
pub async fn spawn_test_listener(port: u16, persona: TestPersona) -> JoinHandle<()> {
    let p: Persona = persona.into();
    tokio::spawn(async move {
        let addr = format!("0.0.0.0:{port}");
        let listener = match sockopt::bind_with_options(&addr, p).await {
            Ok(l) => l,
            Err(e) => {
                eprintln!("[test] failed to bind :{port}: {e}");
                return;
            }
        };
        loop {
            match listener.accept().await {
                Ok((socket, peer)) => {
                    tokio::spawn(personas::handle_connection(socket, peer, p));
                }
                Err(e) => {
                    eprintln!("[test] accept error: {e}");
                    break;
                }
            }
        }
    })
}

/// Spawn a dashboard HTTP server on the given port for test use.
pub async fn spawn_test_dashboard(port: u16) -> JoinHandle<()> {
    let metrics = Metrics::new();
    metrics.set_port(9000);
    tokio::spawn(async move {
        if let Err(e) = dashboard::start_dashboard(metrics, port).await {
            eprintln!("[test] dashboard error: {e}");
        }
    })
}

/// Exposed for integration tests — mirrors validate_port in config.rs.
pub fn config_validate_port_range(port: u16, name: &str, errors: &mut Vec<String>) {
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
