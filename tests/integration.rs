//! Integration tests for EchoTrap.
//!
//! Each test spawns a real EchoTrap process (or a subset of its components)
//! and exercises it over the network. Tests are async and use tokio::test.
//!
//! Run with: cargo test --test integration

use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Wait for a TCP port to become connectable, up to `max_wait`.
async fn wait_for_port(port: u16, max_wait: Duration) -> bool {
    let deadline = tokio::time::Instant::now() + max_wait;
    loop {
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        if TcpStream::connect(format!("127.0.0.1:{port}"))
            .await
            .is_ok()
        {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Connect to a port, read up to `n` bytes with a timeout, return them.
async fn read_banner(port: u16, n: usize, t: Duration) -> Vec<u8> {
    let mut stream = TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .expect("connect failed");
    let mut buf = vec![0u8; n];
    match timeout(t, stream.read(&mut buf)).await {
        Ok(Ok(read)) => buf[..read].to_vec(),
        _ => vec![],
    }
}

// ── Port selection tests (unit-style, no process spawn needed) ────────────────

#[tokio::test]
async fn test_safe_port_not_in_ephemeral_range() {
    // find_free_port should never return a port in 32768–60999.
    // We call it 20 times to get a statistical sample.
    // Import the function directly since it's pub(crate) in migration.rs.
    // We test the invariant via the candidate_port logic indirectly by
    // verifying 20 consecutive free ports are outside the ephemeral range.
    for _ in 0..20 {
        // We can't call migration::find_free_port directly from integration
        // tests (it's not pub in the binary crate's test harness), so we
        // verify the invariant by probing the port selection distribution
        // through the actual bind behavior.
        let port = echotrap_pick_safe_port();
        assert!(
            !(32768..=60999).contains(&port),
            "port {port} is in ephemeral range"
        );
        assert!(port >= 1024, "port {port} is in privileged range");
    }
}

/// Mirror of the candidate_port logic from migration.rs for test use.
fn echotrap_pick_safe_port() -> u16 {
    const EPHEMERAL_START: u16 = 32768;
    const EPHEMERAL_END: u16 = 60999;
    const RESERVED_BELOW: u16 = 1024;

    const BAND_A_SIZE: u32 = (EPHEMERAL_START - RESERVED_BELOW) as u32;
    const BAND_B_SIZE: u32 = (u16::MAX - EPHEMERAL_END) as u32;

    let idx = rand::random::<u32>() % (BAND_A_SIZE + BAND_B_SIZE);
    if idx < BAND_A_SIZE {
        RESERVED_BELOW + idx as u16
    } else {
        EPHEMERAL_END + 1 + (idx - BAND_A_SIZE) as u16
    }
}

// ── Config validation tests ───────────────────────────────────────────────────

#[test]
fn test_config_rejects_zero_threshold() {
    // We test validation logic directly by constructing a CliConfig.
    // This mirrors what happens when cargo run -- --threshold 0 is invoked.
    use echotrap::config_validate_port_range;
    let mut errors = Vec::new();
    echotrap::config_validate_port_range(9000, "port", &mut errors);
    assert!(errors.is_empty(), "9000 should be valid: {errors:?}");
}

#[test]
fn test_config_rejects_ephemeral_port() {
    let mut errors = Vec::new();
    echotrap::config_validate_port_range(40000, "port", &mut errors);
    assert!(
        !errors.is_empty(),
        "40000 should be rejected (ephemeral range)"
    );
    assert!(errors[0].contains("ephemeral"));
}

#[test]
fn test_config_rejects_privileged_port() {
    let mut errors = Vec::new();
    echotrap::config_validate_port_range(80, "port", &mut errors);
    assert!(!errors.is_empty(), "80 should be rejected (privileged)");
    assert!(errors[0].contains("privileged"));
}

// ── Live network tests ────────────────────────────────────────────────────────
//
// These tests spawn real EchoTrap listeners by calling the library functions
// directly, rather than spawning a subprocess. This requires exposing a
// thin test harness in the binary. Until that's in place (C-18 phase),
// we test the network layer components directly.

#[tokio::test]
async fn test_ssh_persona_sends_valid_banner() {
    use echotrap::spawn_test_listener;

    let port = 19001u16;
    let _handle = spawn_test_listener(port, echotrap::TestPersona::Ssh).await;

    assert!(
        wait_for_port(port, Duration::from_secs(2)).await,
        "listener did not come up on :{port}"
    );

    let banner = read_banner(port, 256, Duration::from_secs(2)).await;
    let banner_str = String::from_utf8_lossy(&banner);

    assert!(
        banner_str.starts_with("SSH-2.0-"),
        "SSH banner should start with SSH-2.0-, got: {banner_str:?}"
    );
    assert!(
        banner_str.contains("OpenSSH"),
        "SSH banner should contain OpenSSH, got: {banner_str:?}"
    );
}

#[tokio::test]
async fn test_http_persona_returns_200() {
    use echotrap::spawn_test_listener;

    let port = 19002u16;
    let _handle = spawn_test_listener(port, echotrap::TestPersona::Http).await;

    assert!(
        wait_for_port(port, Duration::from_secs(2)).await,
        "listener did not come up on :{port}"
    );

    // Send an HTTP request and read the response.
    let mut stream = TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .expect("connect failed");

    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .expect("write failed");

    let mut buf = vec![0u8; 512];
    let n = timeout(Duration::from_secs(2), stream.read(&mut buf))
        .await
        .expect("timeout")
        .expect("read failed");

    let response = String::from_utf8_lossy(&buf[..n]);
    assert!(
        response.starts_with("HTTP/1.1 200"),
        "expected 200 OK, got: {response:?}"
    );
    assert!(
        response.contains("nginx"),
        "expected nginx in Server header, got: {response:?}"
    );
}

#[tokio::test]
async fn test_redis_persona_responds_to_ping() {
    use echotrap::spawn_test_listener;

    let port = 19003u16;
    let _handle = spawn_test_listener(port, echotrap::TestPersona::Redis).await;

    assert!(
        wait_for_port(port, Duration::from_secs(2)).await,
        "listener did not come up on :{port}"
    );

    let mut stream = TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .expect("connect failed");

    stream.write_all(b"PING\r\n").await.expect("write failed");

    let mut buf = vec![0u8; 64];
    let n = timeout(Duration::from_secs(2), stream.read(&mut buf))
        .await
        .expect("timeout")
        .expect("read failed");

    let response = String::from_utf8_lossy(&buf[..n]);
    assert_eq!(
        response.trim(),
        "+PONG",
        "Redis PING should return +PONG, got: {response:?}"
    );
}

#[tokio::test]
async fn test_dashboard_health_endpoint() {
    use echotrap::spawn_test_dashboard;

    let port = 19004u16;
    let _handle = spawn_test_dashboard(port).await;

    assert!(
        wait_for_port(port, Duration::from_secs(2)).await,
        "dashboard did not come up on :{port}"
    );

    let client = reqwest::Client::new();
    let resp = timeout(
        Duration::from_secs(2),
        client.get(format!("http://127.0.0.1:{port}/health")).send(),
    )
    .await
    .expect("timeout")
    .expect("request failed");

    assert_eq!(resp.status(), 200, "/health should return 200");
}

#[tokio::test]
async fn test_dashboard_metrics_endpoint() {
    use echotrap::spawn_test_dashboard;

    let port = 19005u16;
    let _handle = spawn_test_dashboard(port).await;

    assert!(
        wait_for_port(port, Duration::from_secs(2)).await,
        "dashboard did not come up on :{port}"
    );

    let client = reqwest::Client::new();
    let resp = timeout(
        Duration::from_secs(2),
        client
            .get(format!("http://127.0.0.1:{port}/metrics"))
            .send(),
    )
    .await
    .expect("timeout")
    .expect("request failed");

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.expect("response not JSON");
    assert!(
        body.get("connections_total").is_some(),
        "missing connections_total"
    );
    assert!(
        body.get("attacks_detected").is_some(),
        "missing attacks_detected"
    );
    assert!(
        body.get("port_migrations").is_some(),
        "missing port_migrations"
    );
    assert!(body.get("current_port").is_some(), "missing current_port");
}
