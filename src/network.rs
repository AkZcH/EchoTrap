// src/network.rs
use crate::config::CliConfig;
use crate::metrics::Metrics;
use crate::migration;
use crate::personas;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;
use tokio::sync::{broadcast, Mutex, Semaphore};
use tracing::{error, info, warn};

const MIGRATION_COOLDOWN_SECS: u64 = 5;
const DECOY_DURATION_SECS: u64 = 30;
const DRAIN_TIMEOUT_SECS: u64 = 5;
/// How long to wait for a semaphore permit before dropping the connection.
const BACKPRESSURE_WAIT_MS: u64 = 500;

pub async fn start_listener(cfg: CliConfig, metrics: Arc<Metrics>) {
    metrics.set_port(cfg.port);

    let tracker = Arc::new(Mutex::new(crate::detector::AttackTracker::new(
        cfg.threshold as usize,
        cfg.window,
    )));

    // Semaphore caps concurrent active handlers. Shared across all listener
    // tasks (initial + post-migration) so the limit is global, not per-port.
    let max_conn = NonZeroUsize::new(cfg.max_connections)
        .expect("max_connections validated > 0")
        .get();
    let semaphore: Arc<Semaphore> = Arc::new(Semaphore::new(max_conn));

    let shutdown_tx = Arc::new(Mutex::new({
        let (tx, _rx) = broadcast::channel::<()>(1);
        tx
    }));
    let last_migration: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
    let current_port = Arc::new(Mutex::new(cfg.port));
    let active_connections: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));

    let (migrate_req_tx, mut migrate_req_rx) = tokio::sync::mpsc::channel::<()>(8);

    {
        let (new_tx, _new_rx) = broadcast::channel::<()>(1);
        *shutdown_tx.lock().await = new_tx.clone();
        spawn_listener_with_migrate(
            cfg.port,
            cfg.clone(),
            tracker.clone(),
            metrics.clone(),
            active_connections.clone(),
            semaphore.clone(),
            new_tx.subscribe(),
            migrate_req_tx.clone(),
        );
    }

    let shutdown_tx_clone = shutdown_tx.clone();
    let cfg_clone = cfg.clone();
    let tracker_clone = tracker.clone();
    let metrics_clone = metrics.clone();
    let active_connections_clone = active_connections.clone();
    let semaphore_clone = semaphore.clone();
    let last_migration_clone = last_migration.clone();
    let current_port_clone = current_port.clone();

    let migration_handle = tokio::spawn(async move {
        while migrate_req_rx.recv().await.is_some() {
            let now = Instant::now();
            let mut lm = last_migration_clone.lock().await;
            if let Some(last) = *lm {
                if now.duration_since(last) < Duration::from_secs(MIGRATION_COOLDOWN_SECS) {
                    info!("Migration request ignored — cooldown active");
                    continue;
                }
            }

            let old_port = *current_port_clone.lock().await;
            let new_port = match migration::find_free_port(old_port).await {
                Ok(p) => p,
                Err(e) => {
                    warn!("Migration aborted — {e}");
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
                active_connections_clone.clone(),
                semaphore_clone.clone(),
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

            let decoy_banner = cfg_clone.persona.banner_str();
            tokio::spawn(async move {
                // On Linux with nft available: add REDIRECT rule for zero-downtime
                // migration, then hand off to decoy after REDIRECT_DURATION.
                // On Windows or when nft is unavailable: falls back to decoy-only
                // with the existing 200ms settle window.
                #[cfg(target_os = "linux")]
                {
                    crate::redirect::spawn_redirect_then_decoy(
                        old_port,
                        new_port,
                        decoy_banner,
                        Duration::from_secs(DECOY_DURATION_SECS),
                    );
                }
                #[cfg(not(target_os = "linux"))]
                {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    migration::spawn_decoy(
                        old_port,
                        decoy_banner,
                        Duration::from_secs(DECOY_DURATION_SECS),
                    );
                }
            });

            *lm = Some(Instant::now());
            info!("Migration complete — listening on :{new_port}");
        }
    });

    match tokio::signal::ctrl_c().await {
        Ok(()) => info!("Ctrl-C received — shutting down gracefully"),
        Err(e) => error!("Failed to listen for Ctrl-C signal: {e}"),
    }

    migration_handle.abort();

    {
        let tx = shutdown_tx.lock().await;
        let _ = tx.send(());
    }

    info!("Waiting up to {DRAIN_TIMEOUT_SECS}s for in-flight connections to close...");
    let drain_deadline = tokio::time::Instant::now() + Duration::from_secs(DRAIN_TIMEOUT_SECS);

    loop {
        let remaining = drain_deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            let still_open = *active_connections.lock().await;
            if still_open > 0 {
                warn!("{still_open} connection(s) still open after drain timeout — forcing exit");
            }
            break;
        }
        if *active_connections.lock().await == 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    info!("EchoTrap shut down cleanly. Goodbye.");
}

#[allow(clippy::too_many_arguments)]
fn spawn_listener_with_migrate(
    port: u16,
    cfg: CliConfig,
    tracker: Arc<Mutex<crate::detector::AttackTracker>>,
    metrics: Arc<Metrics>,
    active_connections: Arc<Mutex<usize>>,
    semaphore: Arc<Semaphore>,
    mut shutdown_rx: broadcast::Receiver<()>,
    migrate_req_tx: tokio::sync::mpsc::Sender<()>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let bind_addr = format!("0.0.0.0:{port}");
        info!("Spawning listener on {bind_addr}");

        let listener = match crate::sockopt::bind_with_options(&bind_addr, cfg.persona).await {
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
                            info!("Accepted connection from {peer} [persona: {}]", cfg.persona);
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

                            // Try to acquire a semaphore permit within the
                            // backpressure window. If full, drop with graceful
                            // FIN — not RST, which is a scanner signal.
                            let permit = match tokio::time::timeout(
                                Duration::from_millis(BACKPRESSURE_WAIT_MS),
                                semaphore.clone().acquire_owned(),
                            ).await {
                                Ok(Ok(p)) => p,
                                Ok(Err(_)) => {
                                    // Semaphore closed — shutdown in progress.
                                    break;
                                }
                                Err(_) => {
                                    // Timeout — at capacity, drop gracefully.
                                    warn!(
                                        "Connection limit reached ({} max) — dropping {peer} with FIN",
                                        cfg.max_connections
                                    );
                                    let mut s = socket;
                                    let _ = s.shutdown().await;
                                    continue;
                                }
                            };

                            *active_connections.lock().await += 1;
                            let ac = active_connections.clone();
                            let persona = cfg.persona;
                            tokio::spawn(async move {
                                // Permit is held for the lifetime of the handler.
                                let _permit = permit;
                                personas::handle_connection(socket, peer, persona).await;
                                *ac.lock().await -= 1;
                            });
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
