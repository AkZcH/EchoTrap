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

use crate::error::SockoptError;
use crate::persona::Persona;
use socket2::{Domain, Protocol, Socket, TcpKeepalive, Type};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpListener;
use tracing::warn;

/// Build a `TcpListener` with socket options tuned to match the given persona's
/// expected OS fingerprint.
pub async fn bind_with_options(addr: &str, persona: Persona) -> Result<TcpListener, SockoptError> {
    let socket_addr: SocketAddr =
        addr.parse()
            .map_err(|e: std::net::AddrParseError| SockoptError::Bind {
                addr: addr.to_string(),
                source: std::io::Error::new(std::io::ErrorKind::InvalidInput, e.to_string()),
            })?;

    let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))
        .map_err(SockoptError::Create)?;

    socket
        .set_reuse_address(true)
        .map_err(SockoptError::Create)?;

    // Per-persona socket options — failures are warned but non-fatal.
    // A socket option failure doesn't prevent the honeypot from starting;
    // it just means the fingerprint resistance for that option is degraded.
    apply_persona_options(&socket, persona);

    socket
        .bind(&socket_addr.into())
        .map_err(|e| SockoptError::Bind {
            addr: addr.to_string(),
            source: e,
        })?;

    socket.listen(1024).map_err(|e| SockoptError::Bind {
        addr: addr.to_string(),
        source: e,
    })?;

    socket.set_nonblocking(true).map_err(SockoptError::Create)?;

    let std_listener: std::net::TcpListener = socket.into();
    TcpListener::from_std(std_listener).map_err(SockoptError::Convert)
}

fn apply_persona_options(socket: &Socket, persona: Persona) {
    match persona {
        Persona::Ssh => apply_ssh_options(socket),
        Persona::Http => apply_http_options(socket),
        Persona::Redis => apply_redis_options(socket),
        Persona::Raw => apply_raw_options(socket),
    }
}

fn apply_ssh_options(socket: &Socket) {
    if let Err(e) = socket.set_recv_buffer_size(131072) {
        warn!("[sockopt] Failed to set recv buffer for SSH persona: {e}");
    }
    if let Err(e) = socket.set_keepalive(true) {
        warn!("[sockopt] Failed to set SO_KEEPALIVE for SSH persona: {e}");
    }
    let keepalive = TcpKeepalive::new()
        .with_time(Duration::from_secs(120))
        .with_interval(Duration::from_secs(10));
    if let Err(e) = socket.set_tcp_keepalive(&keepalive) {
        warn!("[sockopt] Failed to set TCP keepalive params for SSH persona: {e}");
    }
    if let Err(e) = socket.set_nodelay(false) {
        warn!("[sockopt] Failed to set TCP_NODELAY for SSH persona: {e}");
    }
}

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

fn apply_raw_options(socket: &Socket) {
    if let Err(e) = socket.set_nodelay(true) {
        warn!("[sockopt] Failed to set TCP_NODELAY for Raw persona: {e}");
    }
}
