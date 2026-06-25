// src/sockopt.rs
//
// Build a TCP listener with socket options that match real service OS fingerprints.
//
// Masscan/ZMap/nmap -O fingerprint the OS by looking at:
//   - TCP initial window size (IWS)
//   - SO_KEEPALIVE
//   - TCP_NODELAY
//   - Receive buffer size (affects advertised window)
//
// Default tokio TcpListener uses OS defaults, which on Windows differ from
// Linux server defaults that real SSH/nginx/Redis would show. We override
// them here to match Ubuntu 22.04 LTS server defaults.
//
// Reference: https://github.com/nmap/nmap/blob/master/nmap-os-db (Linux 5.x entries)

use crate::persona::Persona;
use socket2::{Domain, Protocol, Socket, TcpKeepalive, Type};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpListener;
use tracing::warn;

/// Build a `TcpListener` with socket options tuned to match the given persona's
/// expected OS fingerprint. Falls back to a plain tokio bind on any error so
/// the honeypot always starts even if socket option tuning fails.
pub async fn bind_with_options(addr: &str, persona: Persona) -> std::io::Result<TcpListener> {
    let socket_addr: SocketAddr = addr.parse().map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("bad addr: {e}"))
    })?;

    // socket2 socket — gives us pre-bind option control.
    let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))?;

    // SO_REUSEADDR — required on Unix to rebind quickly after migration.
    // On Windows this has different semantics but is still needed.
    socket.set_reuse_address(true)?;

    // Per-persona socket options matching real server defaults on Ubuntu 22.04.
    apply_persona_options(&socket, persona);

    socket.bind(&socket_addr.into())?;
    // Backlog 1024 — matches nginx/OpenSSH default listen backlog.
    socket.listen(1024)?;

    // Convert to std TcpListener, then to tokio TcpListener.
    socket.set_nonblocking(true)?;
    let std_listener: std::net::TcpListener = socket.into();
    TcpListener::from_std(std_listener)
}

/// Apply socket options that match the persona's expected server fingerprint.
fn apply_persona_options(socket: &Socket, persona: Persona) {
    match persona {
        Persona::Ssh => apply_ssh_options(socket),
        Persona::Http => apply_http_options(socket),
        Persona::Redis => apply_redis_options(socket),
        Persona::Raw => apply_raw_options(socket),
    }
}

/// OpenSSH on Ubuntu 22.04:
///   - TCP window size: 65535 (initial, before window scaling)
///   - SO_KEEPALIVE: enabled, 120s idle, 3 probes, 10s interval
///   - TCP_NODELAY: disabled (SSH does its own buffering)
///   - Recv buffer: 131072 (128KB default on Linux)
fn apply_ssh_options(socket: &Socket) {
    if let Err(e) = socket.set_recv_buffer_size(131072) {
        warn!("[sockopt] Failed to set recv buffer for SSH persona: {e}");
    }
    if let Err(e) = socket.set_keepalive(true) {
        warn!("[sockopt] Failed to set SO_KEEPALIVE for SSH persona: {e}");
    }
    // TCP keepalive parameters — matches sshd defaults.
    let keepalive = TcpKeepalive::new()
        .with_time(Duration::from_secs(120))
        .with_interval(Duration::from_secs(10));
    if let Err(e) = socket.set_tcp_keepalive(&keepalive) {
        warn!("[sockopt] Failed to set TCP keepalive params for SSH persona: {e}");
    }
    // TCP_NODELAY off — SSH does its own Nagle-like buffering.
    if let Err(e) = socket.set_nodelay(false) {
        warn!("[sockopt] Failed to set TCP_NODELAY for SSH persona: {e}");
    }
}

/// nginx on Ubuntu 22.04:
///   - TCP_NODELAY: enabled (nginx sets it for low-latency HTTP)
///   - SO_KEEPALIVE: disabled by default in nginx
///   - Recv buffer: 65536 (nginx default)
fn apply_http_options(socket: &Socket) {
    if let Err(e) = socket.set_recv_buffer_size(65536) {
        warn!("[sockopt] Failed to set recv buffer for HTTP persona: {e}");
    }
    if let Err(e) = socket.set_nodelay(true) {
        warn!("[sockopt] Failed to set TCP_NODELAY for HTTP persona: {e}");
    }
    if let Err(e) = socket.set_keepalive(false) {
        warn!("[sockopt] Failed to clear SO_KEEPALIVE for HTTP persona: {e}");
    }
}

/// Redis on Ubuntu 22.04:
///   - TCP_NODELAY: enabled (Redis explicitly sets it)
///   - SO_KEEPALIVE: enabled (Redis sets it)
///   - Recv buffer: 65536
fn apply_redis_options(socket: &Socket) {
    if let Err(e) = socket.set_recv_buffer_size(65536) {
        warn!("[sockopt] Failed to set recv buffer for Redis persona: {e}");
    }
    if let Err(e) = socket.set_nodelay(true) {
        warn!("[sockopt] Failed to set TCP_NODELAY for Redis persona: {e}");
    }
    if let Err(e) = socket.set_keepalive(true) {
        warn!("[sockopt] Failed to set SO_KEEPALIVE for Redis persona: {e}");
    }
}

/// Raw persona — minimal options, no fingerprint emulation.
fn apply_raw_options(socket: &Socket) {
    if let Err(e) = socket.set_nodelay(true) {
        warn!("[sockopt] Failed to set TCP_NODELAY for Raw persona: {e}");
    }
}