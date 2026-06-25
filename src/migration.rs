// src/migration.rs
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tracing::{info, warn};

// ── Port selection ────────────────────────────────────────────────────────────

const EPHEMERAL_START: u16 = 32768;
const EPHEMERAL_END: u16 = 60999;
const RESERVED_BELOW: u16 = 1024;
const MAX_PORT_ATTEMPTS: usize = 16;

fn candidate_port() -> u16 {
    const BAND_A_SIZE: u32 = (EPHEMERAL_START - RESERVED_BELOW) as u32;
    const BAND_B_SIZE: u32 = (u16::MAX - EPHEMERAL_END) as u32;

    let idx = rand::random::<u32>() % (BAND_A_SIZE + BAND_B_SIZE);
    if idx < BAND_A_SIZE {
        RESERVED_BELOW + idx as u16
    } else {
        EPHEMERAL_END + 1 + (idx - BAND_A_SIZE) as u16
    }
}

pub async fn find_free_port(exclude: u16) -> Option<u16> {
    for _ in 0..MAX_PORT_ATTEMPTS {
        let port = candidate_port();
        if port == exclude {
            continue;
        }
        match TcpListener::bind(format!("0.0.0.0:{port}")).await {
            Ok(_) => return Some(port),
            Err(_) => continue,
        }
    }
    None
}

// ── Migration history ─────────────────────────────────────────────────────────
// Fields and methods are unused now; wired in C-13 (metrics/dashboard).

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct MigrationEvent {
    pub from_port: u16,
    pub to_port: u16,
    pub trigger_ip: Option<SocketAddr>,
    pub at: Instant,
}

#[allow(dead_code)]
#[derive(Debug, Default)]
pub struct MigrationHistory {
    events: Vec<MigrationEvent>,
}

#[allow(dead_code)]
impl MigrationHistory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, from: u16, to: u16, trigger: Option<SocketAddr>) {
        if self.events.len() >= 256 {
            self.events.remove(0);
        }
        self.events.push(MigrationEvent {
            from_port: from,
            to_port: to,
            trigger_ip: trigger,
            at: Instant::now(),
        });
    }

    pub fn last(&self) -> Option<&MigrationEvent> {
        self.events.last()
    }

    pub fn count(&self) -> usize {
        self.events.len()
    }
}

// ── Decoy task ────────────────────────────────────────────────────────────────

pub fn spawn_decoy(old_port: u16, banner: &'static str, duration: Duration) {
    tokio::spawn(async move {
        let listener = match TcpListener::bind(format!("0.0.0.0:{old_port}")).await {
            Ok(l) => {
                info!(
                    "[DECOY] Decoy listener active on :{old_port} for {}s",
                    duration.as_secs()
                );
                l
            }
            Err(e) => {
                warn!("[DECOY] Could not bind decoy on :{old_port}: {e}");
                return;
            }
        };

        let deadline = tokio::time::Instant::now() + duration;

        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }

            match tokio::time::timeout(remaining, listener.accept()).await {
                Ok(Ok((mut socket, peer))) => {
                    tokio::spawn(async move {
                        info!("[DECOY] Scanner {peer} probing old port — feeding dead banner");
                        let _ = socket.write_all(banner.as_bytes()).await;
                        let _ = socket.flush().await;
                        let _ = socket.shutdown().await;
                    });
                }
                Ok(Err(e)) => {
                    warn!("[DECOY] Accept error on decoy :{old_port}: {e}");
                    break;
                }
                Err(_) => break, // deadline elapsed
            }
        }

        info!("[DECOY] Decoy on :{old_port} expired");
    });
}
