# Changelog

## [0.2.0] — 2026-06-29

### Added

**Fingerprint resistance**

- Protocol personas: SSH (OpenSSH 8.9p1), HTTP (nginx 1.18), Redis 7, Raw echo
- Per-protocol timing jitter (SSH: 20–150ms, HTTP: 5–80ms, Redis: 1–10ms)
- TCP socket options via `socket2` — `SO_KEEPALIVE`, `TCP_NODELAY`, recv buffer tuned to match Ubuntu 22.04 server defaults per persona
- Graceful FIN on all connection closes — never RST (RST is a scanner fingerprint signal)

**Port migration**

- Safe port selection avoiding Linux ephemeral range (32768–60999) and privileged range (<1024)
- Probe-bind to verify port is free before migration attempt
- Decoy listener on old port for 30s post-migration — scanners keep probing dead air
- **nftables REDIRECT** (Linux): kernel-level transparent redirect from old port to new port — zero dropped connections during migration window
- `net.ipv4.conf.lo.route_localnet=1` set automatically for loopback NAT support

**Observability**

- HTTP dashboard: `/metrics` (JSON), `/metrics/prometheus` (Prometheus text format 0.0.4), `/status`, `/health`
- Dual-layer logging: HERALD-styled terminal output + NDJSON file for SIEM ingestion
- Prometheus scrape config (`prometheus.yml`) included

**Reliability**

- LRU-bounded attack tracker — hard cap at 10k IPs (~720KB), no unbounded memory growth under scan floods
- Semaphore-based connection cap (default 10k) — excess dropped with graceful FIN
- Graceful shutdown on Ctrl-C with 5s drain window for in-flight connections
- Structured error types via `thiserror` — `DashboardError`, `MigrationError`, `SockoptError`
- Config validation before startup — rejects invalid ports, thresholds, log paths

**Testing & benchmarks**

- Integration test suite: 10 tests covering all personas, dashboard endpoints, Prometheus format, port safety, config validation
- Criterion benchmark harness — throughput, migration latency, detector overhead

**Deployment**

- Docker: two-stage build (rust:1.82-slim → debian:bookworm-slim), non-root user, health check
- GitHub Actions CI: fmt, clippy -D warnings, test, Docker smoke test

### Performance (Linux/WSL2, release build)

| Benchmark                   | Result          |
| --------------------------- | --------------- |
| Connection throughput (100) | ~13,700 conn/s  |
| Connection throughput (500) | ~34,800 conn/s  |
| Connection throughput (1k)  | ~36,400 conn/s  |
| Migration latency           | ~106µs          |
| Detector overhead (single)  | ~59ns per call  |
| Detector overhead (1k IPs)  | ~70ns per call  |
| Detector overhead (thresh)  | ~147ns per call |

## [0.1.0] — 2026-06-23

Initial prototype: async TCP listener, sliding-window attack detection, basic port migration, echo service.
