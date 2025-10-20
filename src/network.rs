// src/network.rs
use crate::config::CliConfig;
use crate::detector::AttackTracker;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex};
use tracing::{error, info, warn};

const BANNER: &str = "Welcome to EchoTrap Service v1.2\r\n";
const MIGRATION_COOLDOWN_SECS: u64 = 5; // avoid thrashing on repeated alerts

/// Spawn a listener accept loop. Returns the JoinHandle (for debugging) but we don't await it.
/// The listener stops when `shutdown_rx` receives a message.
fn spawn_listener(
    port: u16,
    cfg: CliConfig,
    tracker: Arc<Mutex<AttackTracker>>,
    mut shutdown_rx: broadcast::Receiver<()>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let bind_addr = format!("0.0.0.0:{}", port);
        info!("Spawning listener on {}", bind_addr);

        let listener = match TcpListener::bind(&bind_addr).await {
            Ok(l) => {
                info!("EchoTrap listening on {}", bind_addr);
                l
            }
            Err(e) => {
                error!("Failed to bind to {}: {}", bind_addr, e);
                return;
            }
        };

        loop {
            tokio::select! {
                accept_res = listener.accept() => {
                    match accept_res {
                        Ok((socket, peer)) => {
                            info!("Accepted connection from {}", peer);

                            // detection: record and check
                            {
                                let mut guard = tracker.lock().await;
                                let is_suspicious = guard.record_and_check(peer);

                                if is_suspicious {
                                    warn!(
                                        "[ALERT] Port scan/brute-force suspected from {} — {} hits within {}s",
                                        peer, cfg.threshold, cfg.window
                                    );
                                    // Note: we intentionally do not migrate directly here.
                                    // Migration is orchestrated by the manager in `start_listener`.
                                }
                            }

                            // Spawn a handler for the connection (echo)
                            tokio::spawn(handle_connection(socket, peer));
                        }
                        Err(e) => {
                            warn!("Accept error on {}: {}", bind_addr, e);
                        }
                    }
                }

                // Shutdown signal received -> break accept loop
                _ = shutdown_rx.recv() => {
                    info!("Shutdown signal received for listener on {} — stopping accept loop", bind_addr);
                    break;
                }
            }
        }

        info!("Listener on {} has shut down (accept loop ended).", bind_addr);
    })
}

/// Start manager that owns current listener and can migrate it on demand.
/// This function runs the initial listener and reacts to migration calls triggered
/// by `attempt_migration()` which is invoked in the accept loop when suspiciousness is detected.
pub async fn start_listener(cfg: CliConfig) {
    // Shared tracker for detection
    let tracker = Arc::new(Mutex::new(AttackTracker::new(
        cfg.threshold as usize,
        cfg.window,
    )));

    // Shared state for current shutdown sender and last migration time
    let shutdown_tx = Arc::new(Mutex::new({
        // initial channel
        let (tx, _rx) = broadcast::channel::<()>(1);
        tx
    }));
    let last_migration = Arc::new(Mutex::new(Instant::now() - Duration::from_secs(3600))); // old time so first migration allowed

    // Migration request channel
    let (migrate_req_tx, mut migrate_req_rx) = tokio::sync::mpsc::channel::<()>(8);

    // Spawn initial listener with migration support
    {
        let (new_tx, _new_rx) = broadcast::channel::<()>(1);
        *shutdown_tx.lock().await = new_tx.clone();
        spawn_listener_with_migrate(
            cfg.port,
            cfg.clone(),
            tracker.clone(),
            new_tx.subscribe(),
            migrate_req_tx.clone(),
        );
    }

    // Migration executor task: performs migrations requested by accept loops.
    let shutdown_tx_clone = shutdown_tx.clone();
    let cfg_clone = cfg.clone();
    let tracker_clone = tracker.clone();
    let last_migration_clone = last_migration.clone();
    tokio::spawn(async move {
        while let Some(_) = migrate_req_rx.recv().await {
            // Check cooldown
            let now = Instant::now();
            let mut lm = last_migration_clone.lock().await;
            if now.duration_since(*lm) < Duration::from_secs(MIGRATION_COOLDOWN_SECS) {
                info!("Migration request ignored due to cooldown");
                continue;
            }

            // Compute new port
            let new_port: u16 = rand::random::<u16>() % 50_000 + 10_000;

            println!("2025-10-20T10:32:19.114652Z  INFO Migration requested — attempting to move to port {}", new_port);

            // Create new channel & spawn new listener
            let (new_tx, _new_rx) = broadcast::channel::<()>(1);
            {
                // Spawn the new listener
                spawn_listener_with_migrate(
                    new_port,
                    cfg_clone.clone(),
                    tracker_clone.clone(),
                    new_tx.subscribe(),
                    migrate_req_tx.clone(),
                );

                // Swap the shutdown sender: send shutdown to old one, then replace it
                let mut tx_guard = shutdown_tx_clone.lock().await;
                // send shutdown to old listener (best-effort)
                let _ = tx_guard.send(());
                // replace
                *tx_guard = new_tx.clone();
            }

            // Update last_migration
            *lm = Instant::now();
            println!("2025-10-20T10:32:19.114652Z  INFO Migration completed: new listener on port {}", new_port);
        }
    });

    // The manager task will just await forever (or until process exit).
    futures::future::pending::<()>().await;
}

/// A variant of spawn_listener which accepts a migration-request sender.
/// When it detects a suspicious IP it will attempt to request a migration by sending on migrate_req_tx.
/// This keeps detection + migration decoupled (accept loop requests migration; manager performs it).
fn spawn_listener_with_migrate(
    port: u16,
    cfg: CliConfig,
    tracker: Arc<Mutex<AttackTracker>>,
    mut shutdown_rx: broadcast::Receiver<()>,
    migrate_req_tx: tokio::sync::mpsc::Sender<()>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let bind_addr = format!("0.0.0.0:{}", port);
        println!("2025-10-20T10:32:19.114652Z  INFO Spawning listener on {}", bind_addr);

        let listener = match TcpListener::bind(&bind_addr).await {
            Ok(l) => {
                println!("2025-10-20T10:32:19.114652Z  INFO EchoTrap listening on {}", bind_addr);
                l
            }
            Err(e) => {
                println!("2025-10-20T10:32:19.114652Z ERROR Failed to bind to {}: {}", bind_addr, e);
                return;
            }
        };

        loop {
            tokio::select! {
                accept_res = listener.accept() => {
                    match accept_res {
                        Ok((socket, peer)) => {
                            println!("2025-10-20T10:32:19.114652Z  INFO Accepted connection from {}", peer);

                            // detection: record and check
                            let mut should_request_migration = false;
                            {
                                let mut guard = tracker.lock().await;
                                let is_suspicious = guard.record_and_check(peer);

                                if is_suspicious {
                                    println!(
                                        "2025-10-20T10:32:19.114652Z  WARN [ALERT] Port scan/brute-force suspected from {} — {} hits within {}s",
                                        peer, cfg.threshold, cfg.window
                                    );
                                    // Instead of migrating directly, we set flag to request migration asynchronously
                                    should_request_migration = true;
                                }
                            }

                            // Request migration on a best-effort, non-blocking basis
                            if should_request_migration {
                                // Try to send a migration request without blocking; if channel full, ignore.
                                let _ = migrate_req_tx.try_send(());
                            }

                            // Spawn a handler for the connection (echo)
                            tokio::spawn(handle_connection(socket, peer));
                        }
                        Err(e) => {
                            warn!("Accept error on {}: {}", bind_addr, e);
                        }
                    }
                }

                // Shutdown signal received -> break accept loop
                _ = shutdown_rx.recv() => {
                    info!("Shutdown signal received for listener on {} — stopping accept loop", bind_addr);
                    break;
                }
            }
        }

        info!("Listener on {} has shut down (accept loop ended).", bind_addr);
    })
}

/// Per-connection handler: send banner, then echo anything the client sends
async fn handle_connection(mut socket: TcpStream, peer: SocketAddr) {
    let (reader, mut writer) = socket.split();
    let mut buf_reader = BufReader::new(reader);

    if let Err(e) = writer.write_all(BANNER.as_bytes()).await {
        error!("Failed to send banner to {}: {}", peer, e);
        return;
    }
    if let Err(e) = writer.flush().await {
        warn!("Failed to flush banner to {}: {}", peer, e);
    }

    let mut line = String::new();
    loop {
        line.clear();
        match buf_reader.read_line(&mut line).await {
            Ok(0) => {
                info!("Connection closed by {}", peer);
                return;
            }
            Ok(_n) => {
                let payload = line.trim_end_matches(&['\r', '\n'][..]).to_string();
                info!("[CONN] From {} ({} bytes): {:?}", peer, payload.len(), payload);

                if let Err(e) = writer
                    .write_all(format!("{}\r\n", payload).as_bytes())
                    .await
                {
                    warn!("Failed to write to {}: {}", peer, e);
                    return;
                }
                if let Err(e) = writer.flush().await {
                    warn!("Failed to flush to {}: {}", peer, e);
                    return;
                }
            }
            Err(e) => {
                warn!("Read error from {}: {}", peer, e);
                let _ = writer.shutdown().await;
                return;
            }
        }
    }
}