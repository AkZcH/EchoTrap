// src/personas.rs
// Protocol persona handlers. Persona enum lives in src/persona.rs.

use crate::persona::Persona;
use std::net::SocketAddr;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tracing::{info, warn};

pub async fn handle_connection(socket: TcpStream, peer: SocketAddr, persona: Persona) {
    let (min_ms, max_ms) = persona.jitter_ms();
    if max_ms > 0 {
        let jitter = rand::random::<u64>() % (max_ms - min_ms + 1) + min_ms;
        tokio::time::sleep(tokio::time::Duration::from_millis(jitter)).await;
    }

    match persona {
        Persona::Ssh => handle_ssh(socket, peer).await,
        Persona::Http => handle_http(socket, peer).await,
        Persona::Redis => handle_redis(socket, peer).await,
        Persona::Raw => handle_raw(socket, peer).await,
    }
}

async fn handle_ssh(mut socket: TcpStream, peer: SocketAddr) {
    if let Err(e) = socket.write_all(Persona::Ssh.banner()).await {
        warn!("[SSH] Failed to send banner to {peer}: {e}");
        return;
    }
    let _ = socket.flush().await;

    let mut buf = vec![0u8; 256];
    match tokio::time::timeout(tokio::time::Duration::from_secs(10), socket.read(&mut buf)).await {
        Ok(Ok(n)) if n > 0 => {
            let client_ver = String::from_utf8_lossy(&buf[..n]);
            info!("[SSH] {peer} sent version: {:?}", client_ver.trim());
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }
        Ok(Ok(_)) => info!("[SSH] {peer} closed without sending version"),
        Ok(Err(e)) => warn!("[SSH] Read error from {peer}: {e}"),
        Err(_) => info!("[SSH] {peer} timed out on version exchange"),
    }

    let _ = socket.shutdown().await;
}

const HTTP_RESPONSE: &[u8] = b"HTTP/1.1 200 OK\r\n\
Server: nginx/1.18.0 (Ubuntu)\r\n\
Date: Thu, 01 Jan 2026 00:00:00 GMT\r\n\
Content-Type: text/html\r\n\
Content-Length: 0\r\n\
Connection: close\r\n\
\r\n";

async fn handle_http(mut socket: TcpStream, peer: SocketAddr) {
    let (reader, mut writer) = socket.split();
    let mut buf_reader = BufReader::new(reader);
    let mut request_line = String::new();

    match tokio::time::timeout(
        tokio::time::Duration::from_secs(10),
        buf_reader.read_line(&mut request_line),
    )
    .await
    {
        Ok(Ok(0)) => {
            info!("[HTTP] {peer} closed without request");
            return;
        }
        Ok(Ok(_)) => {
            info!("[HTTP] {peer} request: {:?}", request_line.trim());
        }
        Ok(Err(e)) => {
            warn!("[HTTP] Read error from {peer}: {e}");
            return;
        }
        Err(_) => {
            info!("[HTTP] {peer} timed out");
            return;
        }
    }

    let _ = writer.write_all(HTTP_RESPONSE).await;
    let _ = writer.flush().await;
    let _ = writer.shutdown().await;
    info!("[HTTP] {peer} served 200 OK");
}

async fn handle_redis(mut socket: TcpStream, peer: SocketAddr) {
    let (reader, mut writer) = socket.split();
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();
        match tokio::time::timeout(
            tokio::time::Duration::from_secs(30),
            buf_reader.read_line(&mut line),
        )
        .await
        {
            Ok(Ok(0)) => {
                info!("[Redis] {peer} disconnected");
                return;
            }
            Ok(Ok(_)) => {
                let cmd = line.trim().to_uppercase();
                info!("[Redis] {peer} sent: {cmd:?}");
                let response: &[u8] = if cmd == "PING" || cmd.starts_with("PING ") {
                    b"+PONG\r\n"
                } else if cmd == "QUIT" {
                    let _ = writer.write_all(b"+OK\r\n").await;
                    let _ = writer.flush().await;
                    let _ = writer.shutdown().await;
                    return;
                } else {
                    b"-ERR unknown command\r\n"
                };
                if let Err(e) = writer.write_all(response).await {
                    warn!("[Redis] Write error to {peer}: {e}");
                    return;
                }
                let _ = writer.flush().await;
            }
            Ok(Err(e)) => {
                warn!("[Redis] Read error from {peer}: {e}");
                return;
            }
            Err(_) => {
                info!("[Redis] {peer} idle timeout");
                let _ = writer.shutdown().await;
                return;
            }
        }
    }
}

async fn handle_raw(mut socket: TcpStream, peer: SocketAddr) {
    if let Err(e) = socket.write_all(Persona::Raw.banner()).await {
        warn!("[Raw] Failed to send banner to {peer}: {e}");
        return;
    }
    let _ = socket.flush().await;

    let (reader, mut writer) = socket.split();
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();
        match buf_reader.read_line(&mut line).await {
            Ok(0) => {
                info!("[Raw] Connection closed by {peer}");
                return;
            }
            Ok(_) => {
                let payload = line.trim_end_matches(&['\r', '\n'][..]).to_string();
                info!("[Raw] From {peer} ({} bytes): {payload:?}", payload.len());
                if let Err(e) = writer.write_all(format!("{payload}\r\n").as_bytes()).await {
                    warn!("[Raw] Write error to {peer}: {e}");
                    return;
                }
                let _ = writer.flush().await;
            }
            Err(e) => {
                warn!("[Raw] Read error from {peer}: {e}");
                let _ = writer.shutdown().await;
                return;
            }
        }
    }
}
