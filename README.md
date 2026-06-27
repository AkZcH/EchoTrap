# EchoTrap

**A TCP honeypot that resists Masscan and ZMap fingerprinting.**

Most honeypots are identified in under a second. Masscan looks at banner timing, TCP window size, and echo behavior — any one of these gives it away. EchoTrap fixes all three.

[![Rust](https://img.shields.io/badge/rust-1.82+-orange)](https://www.rust-lang.org)
[![CI](https://github.com/AkZcH/EchoTrap/actions/workflows/ci.yml/badge.svg)](https://github.com/AkZcH/EchoTrap/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

---

## What it does differently

| Technique            | How EchoTrap applies it                                                               |
| -------------------- | ------------------------------------------------------------------------------------- |
| Protocol personas    | Emulates OpenSSH 8.9p1, nginx 1.18, or Redis 7 — not a generic echo server            |
| Timing jitter        | Randomizes banner latency per protocol (SSH: 20–150ms, HTTP: 5–80ms)                  |
| TCP socket options   | Sets `SO_KEEPALIVE`, `TCP_NODELAY`, recv buffer to match Ubuntu 22.04 server defaults |
| Graceful FIN on drop | Never sends RST — RST is a honeypot signal to scanners                                |
| Port migration       | Moves to a new port on scan detection; keeps old port alive with a decoy for 30s      |
| Safe port selection  | Avoids Linux ephemeral range (32768–60999) to prevent bind conflicts                  |

---

## Performance

Benchmarked with `criterion` on a Windows development machine (loopback). Linux numbers expected 3–5x higher.

| Benchmark                        | Result          |
| -------------------------------- | --------------- |
| Connection throughput (100 conn) | ~1,485 conn/s   |
| Connection throughput (500 conn) | ~1,190 conn/s   |
| Connection throughput (1k conn)  | ~1,158 conn/s   |
| Migration latency (full path)    | ~6.3ms p50      |
| Detector overhead (single IP)    | ~161ns per call |
| Detector overhead (1k IPs, LRU)  | ~204ns per call |
| Detector overhead (at threshold) | ~403ns per call |

Migration latency is the full critical path: safe port selection → bind → accept confirmation.
Detector overhead is per `record_and_check` call — runs on every accepted connection.

Run benchmarks yourself:
```bash
cargo bench
```

---

## Quickstart

```bash
git clone https://github.com/AkZcH/EchoTrap.git
cd EchoTrap
cargo clippy -- -D warnings
cargo run --release -- --port 9000 --threshold 3 --window 10
```

**Docker:**
```bash
docker run -p 9000:9000 -p 8081:8081 ghcr.io/akzch/echotrap
# or build locally:
docker build -t echotrap . && docker run -p 9000:9000 -p 8081:8081 echotrap
```

Expected output:

```
  [EchoTrap v0.1.0]
  self-rebuilding TCP honeypot

  port         9000
  threshold    3 hits
  window       10s
  persona      ssh
  log          ./echotrap.log
  max-conn     10000
  dashboard    0.0.0.0:8081
  ────────────────────────────────────────────────
  · Spawning listener on 0.0.0.0:9000
  ✓ EchoTrap listening on 0.0.0.0:9000
  ✓ Dashboard listening on http://0.0.0.0:8081
```

---

## Configuration

```
--port <PORT>              Honeypot TCP port (default: 9000)
--threshold <N>            Hits from one IP before migration (default: 5)
--window <SECS>            Sliding window for detection (default: 10)
--persona <PERSONA>        Protocol to emulate: ssh | http | redis | raw (default: ssh)
--max-connections <N>      Concurrent connection cap (default: 10000)
--dashboard-port <PORT>    HTTP metrics port (default: 8081)
--log <PATH>               Log file path (default: ./echotrap.log)
--config <PATH>            Optional TOML config file (CLI flags override)
```

**TOML config example:**

```toml
port = 9000
threshold = 3
window = 10
persona = "http"
max_connections = 10000
dashboard_port = 8081
log = "./echotrap.log"
```

---

## Personas

**SSH** (default) — sends `SSH-2.0-OpenSSH_8.9p1 Ubuntu-3ubuntu0.6`, reads client version string, closes with FIN. Indistinguishable from a hardened SSH server rejecting a key exchange.

```bash
cargo run --release -- --port 9000 --persona ssh
```

**HTTP** — waits for a request line, responds with nginx/1.18.0 headers and `200 OK`. `curl`, `wget`, and nmap `-sV` all see a real web server.

```bash
cargo run --release -- --port 9000 --persona http
```

**Redis** — responds `+PONG` to `PING`, `-ERR unknown command` to everything else. `redis-cli -p 9000 ping` returns `PONG`.

```bash
cargo run --release -- --port 9000 --persona redis
```

---

## Migration

When an IP exceeds the threshold within the detection window:

1. A new listener is spawned on a safe random port
2. The old listener receives a shutdown signal
3. 200ms later, a **decoy** binds the old port and serves the persona banner for 30s
4. Scanners probing the old port keep getting plausible responses while the real listener is elsewhere

```
  ! [ALERT] Scan suspected from 203.0.113.44 — 3 hits in 10s window
  · Migration requested — moving from :9000 to :21629
  ✓ EchoTrap listening on 0.0.0.0:21629
  ✓ Migration complete — listening on :21629
  · Shutdown signal received on :9000 — stopping
  ✓ [DECOY] Decoy listener active on :9000 for 30s
  · [DECOY] Scanner 203.0.113.44 probing old port — feeding dead banner
```

---

## Observability

**JSON metrics** at `http://localhost:8081/metrics`:

```json
{
  "connections_total": 327,
  "attacks_detected": 5,
  "port_migrations": 2,
  "current_port": 21629,
  "uptime_secs": 412
}
```

**Prometheus** at `http://localhost:8081/metrics/prometheus` — scrape with:

```yaml
# prometheus.yml
scrape_configs:
  - job_name: echotrap
    static_configs:
      - targets: ["localhost:8081"]
    metrics_path: /metrics/prometheus
```

**Structured JSON logs** written to `--log` path for SIEM ingestion:

```json
{"timestamp":"2026-06-26T03:16:50.493151Z","level":"WARN","message":"[ALERT] Scan suspected from 127.0.0.1:62760 — 3 hits in 10s window"}
{"timestamp":"2026-06-26T03:16:50.493745Z","level":"INFO","message":"Migration requested — moving from :9000 to :25069"}
{"timestamp":"2026-06-26T03:16:50.545801Z","level":"INFO","message":"Migration complete — listening on :25069"}
```

---

## Testing

```bash
# Unit + integration tests (10 tests, all network-level)
cargo test --test integration

# Benchmarks
cargo bench
```

Integration tests cover: SSH/HTTP/Redis persona banners, dashboard `/health` and `/metrics`, Prometheus format, port safety invariants, config validation.

---

## Simulate an attack

**Linux/macOS:**

```bash
for i in {1..5}; do nc -zv localhost 9000; sleep 0.1; done
```

**Windows (PowerShell):**

```powershell
1..5 | ForEach-Object {
    $c = New-Object System.Net.Sockets.TcpClient
    $c.Connect('localhost', 9000)
    $c.Close()
    Start-Sleep -Milliseconds 100
}
```

---

## Architecture

```
main.rs          Bootstrap, init, spawn dashboard task
config.rs        CLI (clap) + TOML merge + validation
network.rs       Async accept loop, semaphore rate limit, migration executor
detector.rs      LruCache<IpAddr, Vec<Instant>> sliding-window tracker (10k IP cap)
migration.rs     Safe port selection, decoy listener
persona.rs       Persona enum — banner, jitter, socket option profiles
personas.rs      Per-protocol connection handlers (SSH, HTTP, Redis, Raw)
sockopt.rs       socket2 bind with per-persona TCP options
metrics.rs       AtomicUsize counters shared across tasks
dashboard.rs     Axum HTTP server — /metrics /status /metrics/prometheus /health
logger.rs        Dual-layer tracing: HERALD terminal + NDJSON file output
display.rs       ANSI terminal output (✓ · ! ⚡)
```

**Key design decisions:**

- `LruCache` caps memory at 10k tracked IPs (~720KB worst-case) regardless of scan volume
- New listener spawned and confirmed accepting before old one shuts down — zero dropped connections on migration
- Semaphore-based connection cap drops excess with graceful FIN, not RST
- `socket2` pre-bind configuration — tokio's `TcpListener::bind` doesn't expose socket options
- Dual logging: styled terminal output for operators, NDJSON file for SIEM pipelines

---

## Caveats

- Benchmarked on Windows loopback. Linux production numbers will be significantly higher.
- The decoy listener re-binds the old port after 200ms. Connections arriving in that window will see a refused connection.
- Research/portfolio tool. Do not run on production infrastructure or networks you don't own.

---

## License

MIT — see [LICENSE](LICENSE).

**Maintainer:** Akshat Chauhan · [akshatchauhan.dev@gmail.com](mailto:akshatchauhan.dev@gmail.com) · [github.com/AkZcH](https://github.com/AkZcH)
