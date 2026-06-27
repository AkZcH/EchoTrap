//! EchoTrap benchmark harness.
//!
//! Run with: cargo bench
//! Results saved to: target/criterion/

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tokio::runtime::Runtime;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn wait_for_port_sync(rt: &Runtime, port: u16) {
    rt.block_on(async move {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            if tokio::time::Instant::now() >= deadline {
                panic!("port {port} never became ready");
            }
            if TcpStream::connect(format!("127.0.0.1:{port}"))
                .await
                .is_ok()
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    });
}

// ── 1. Connection throughput ──────────────────────────────────────────────────

fn bench_connection_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    // Pre-spawn all listeners before any benchmarking starts.
    // Each gets a dedicated port — no port reuse, no TIME_WAIT collisions.
    let _h100 = rt.block_on(echotrap::spawn_test_listener(
        19100,
        echotrap::TestPersona::Raw,
    ));
    wait_for_port_sync(&rt, 19100);

    let _h500 = rt.block_on(echotrap::spawn_test_listener(
        19101,
        echotrap::TestPersona::Raw,
    ));
    wait_for_port_sync(&rt, 19101);

    let _h1000 = rt.block_on(echotrap::spawn_test_listener(
        19102,
        echotrap::TestPersona::Raw,
    ));
    wait_for_port_sync(&rt, 19102);

    let mut group = c.benchmark_group("connection_throughput");
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(10);

    let cases: &[(u64, u16)] = &[(100, 19100), (500, 19101), (1000, 19102)];

    for &(conn_count, port) in cases {
        group.throughput(Throughput::Elements(conn_count));
        group.bench_with_input(
            BenchmarkId::from_parameter(conn_count),
            &(conn_count, port),
            |b, &(n, port)| {
                b.to_async(&rt).iter(|| async move {
                    let mut tasks = Vec::with_capacity(n as usize);
                    for _ in 0..n {
                        tasks.push(tokio::spawn(async move {
                            if let Ok(mut s) = TcpStream::connect(format!("127.0.0.1:{port}")).await
                            {
                                let mut buf = [0u8; 64];
                                let _ = s.read(&mut buf).await;
                            }
                        }));
                    }
                    for t in tasks {
                        let _ = t.await;
                    }
                });
            },
        );
    }

    group.finish();

    _h100.abort();
    _h500.abort();
    _h1000.abort();
}

// ── 2. Migration latency ──────────────────────────────────────────────────────

fn bench_migration_latency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("migration_latency");
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(20);

    group.bench_function("find_free_port_and_bind", |b| {
        b.to_async(&rt).iter(|| async {
            let new_port = echotrap::migration::find_free_port(19200)
                .await
                .expect("no free port found");

            let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{new_port}"))
                .await
                .expect("bind failed");

            let connect = TcpStream::connect(format!("127.0.0.1:{new_port}")).await;
            drop(listener);
            connect.expect("new listener not ready");
        });
    });

    group.finish();
}

// ── 3. Detector overhead ──────────────────────────────────────────────────────

fn bench_detector(c: &mut Criterion) {
    use echotrap::detector::AttackTracker;

    let mut group = c.benchmark_group("detector");
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(50);

    group.bench_function("single_ip_below_threshold", |b| {
        let mut tracker = AttackTracker::new(5, 10);
        let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();
        b.iter(|| {
            tracker.record_and_check(addr);
        });
    });

    group.bench_function("1000_distinct_ips", |b| {
        let mut tracker = AttackTracker::new(5, 10);
        let mut ip_idx = 0u32;
        b.iter(|| {
            let ip = std::net::Ipv4Addr::from(0x0a000000 + (ip_idx % 1000));
            let addr = SocketAddr::new(std::net::IpAddr::V4(ip), 12345);
            tracker.record_and_check(addr);
            ip_idx += 1;
        });
    });

    group.bench_function("at_threshold", |b| {
        let mut tracker = AttackTracker::new(3, 10);
        let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();
        b.iter(|| {
            tracker.record_and_check(addr);
            tracker.record_and_check(addr);
            tracker.record_and_check(addr);
        });
    });

    group.finish();
}

// ── Registration ──────────────────────────────────────────────────────────────

criterion_group!(
    benches,
    bench_connection_throughput,
    bench_migration_latency,
    bench_detector,
);
criterion_main!(benches);
