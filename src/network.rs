// src/network.rs
use crate::config::CliConfig;
use crate::detector::AttackTracker;
use crate::metrics::Metrics;
use crate::migration;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex};
use tracing::{error, info, warn};

const BANNER: &str = "Welcome to EchoTrap Service v1.2\r\n";
const MIGRATION_COOLDOWN_SECS: u64 = 5;
const DECOY_DURATION_SECS: u64 = 30;

pub async fn start_listener(cfg: CliConfig, metrics: Arc<Metrics>) {
    metrics.set_port(cfg.port);

    let tracker = Arc::new(Mutex::new(AttackTracker::new(
        cfg.threshold as usize,
        cfg.window,
    )));

    let shutdown_tx = Arc::new(Mutex::new({
        let (tx, _rx) = broadcast::channel::<()>(1);
        tx
    }));
    let last_migration = Arc::new(Mutex::new(
        Instant::now() - Duration::from_secs(3600),
    ));
    let current_port = Arc::new(Mutex::new(cfg.port));

    let (migrate_req_tx, mut migrate_req_rx) = tokio::sync::mpsc::channel::<()>(8);

    {
        let (new_tx, _new_rx) = broadcast::channel::<()>(1);
        *shutdown_tx.lock().await = new_tx.clone();
        spawn_listener_with_migrate(
            cfg.port,
            cfg.clone(),
            tracker.clone(),
            metrics.clone(),
            new_tx.subscribe(),
            migrate_req_tx.clone(),
        );
    }

    let shutdown_tx_clone = shutdown_tx.clone();
    let cfg_clone = cfg.clone();
    let tracker_clone = tracker.clone();
    let metrics_clone = metrics.clone();
    let last_migration_clone = last_migration.clone();
    let current_port_clone = current_port.clone();

    tokio::spawn(async move {
        while migrate_req_rx.recv().await.is_some() {
            let now = Instant::now();
            let mut lm = last_migration_clone.lock().await;
            if now.duration_since(*lm) < Duration::from_secs(MIGRATION_COOLDOWN_SECS) {
                info!("Migration request ignored — cooldown active");
                continue;
            }

            let old_port = *current_port_clone.lock().await;
            let new_port = match migration::find_free_port(old_port).await {
                Some(p) => p,
                None => {
                    warn!("Migration aborted — no free port found after 16 attempts");
                    continue;
                }
            };

            info!("Migration requested — moving from :{old_port} to :{new_port}");

            let (new_tx, _new_rx) = broadcast::channel::<()>(1);
            spawn_listener_with_migrate(
                new_port,
                cfg_clone.clone(),
                tracker_clone.clone(),
                metrics_clone.clone(),
                new_tx.subscribe(),
                migrate_req_tx.clone(),
            );

            tokio::time::sleep(Duration::from_millis(50)).await;

            {
                let mut tx_guard = shutdown_tx_clone.lock().await;
                let _ = tx_guard.send(());
                *tx_guard = new_tx;
            }

            *current_port_clone.lock().await = new_port;
            metrics_clone.set_port(new_port);
            metrics_clone.inc_migrations();

            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(200)).await;
                migration::spawn_decoy(
                    old_port,
                    BANNER,
                    Duration::from_secs(DECOY_DURATION_SECS),
                );
            });

            *lm = Instant::now();
            info!("Migration complete — listening on :{new_port}");
        }
    });

    futures::future::pending::<()>().await;
}

fn spawn_listener_with_migrate(
    port: u16,
    cfg: CliConfig,
    tracker: Arc<Mutex<AttackTracker>>,
    metrics: Arc<Metrics>,
    mut shutdown_rx: broadcast::Receiver<()>,
    migrate_req_tx: tokio::sync::mpsc::Sender<()>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let bind_addr = format!("0.0.0.0:{port}");
        info!("Spawning listener on {bind_addr}");

        let listener = match TcpListener::bind(&bind_addr).await {
            Ok(l) => {
                info!("EchoTrap listening on {bind_addr}");
                l
            }
            Err(e) => {
                error!("Failed to bind to {bind_addr}: {e}");
                return;
            }
        };

        loop {
            tokio::select! {
                accept_res = listener.accept() => {
                    match accept_res {
                        Ok((socket, peer)) => {
                            info!("Accepted connection from {peer}");
                            metrics.inc_connections();

                            let mut should_migrate = false;
                            {
                                let mut guard = tracker.lock().await;
                                if guard.record_and_check(peer) {
                                    warn!(
                                        "[ALERT] Scan suspected from {peer} — {} hits in {}s window",
                                        cfg.threshold, cfg.window
                                    );
                                    metrics.inc_attacks();
                                    should_migrate = true;
                                }
                            }

                            if should_migrate {
                                let _ = migrate_req_tx.try_send(());
                            }

                            tokio::spawn(handle_connection(socket, peer));
                        }
                        Err(e) => {
                            warn!("Accept error on {bind_addr}: {e}");
                        }
                    }
                }

                _ = shutdown_rx.recv() => {
                    info!("Shutdown signal received on :{port} — stopping");
                    break;
                }
            }
        }

        info!("Listener on :{port} shut down");
    })
}

async fn handle_connection(mut socket: TcpStream, peer: SocketAddr) {
    let (reader, mut writer) = socket.split();
    let mut buf_reader = BufReader::new(reader);

    if let Err(e) = writer.write_all(BANNER.as_bytes()).await {
        error!("Failed to send banner to {peer}: {e}");
        return;
    }
    if let Err(e) = writer.flush().await {
        warn!("Failed to flush banner to {peer}: {e}");
    }

    let mut line = String::new();
    loop {
        line.clear();
        match buf_reader.read_line(&mut line).await {
            Ok(0) => {
                info!("Connection closed by {peer}");
                return;
            }
            Ok(_) => {
                let payload = line.trim_end_matches(&['\r', '\n'][..]).to_string();
                info!("[CONN] From {peer} ({} bytes): {payload:?}", payload.len());

                if let Err(e) = writer.write_all(format!("{payload}\r\n").as_bytes()).await {
                    warn!("Failed to write to {peer}: {e}");
                    return;
                }
                if let Err(e) = writer.flush().await {
                    warn!("Failed to flush to {peer}: {e}");
                    return;
                }
            }
            Err(e) => {
                warn!("Read error from {peer}: {e}");
                let _ = writer.shutdown().await;
                return;
            }
        }
    }
}