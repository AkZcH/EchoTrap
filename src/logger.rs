// src/logger.rs
use crate::display;
use std::io::Write;
use tracing::Level;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::EnvFilter;

struct DisplayWriter {
    level: Level,
}

impl Write for DisplayWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let raw = std::str::from_utf8(buf)
            .unwrap_or("")
            .trim_end_matches('\n');

        if raw.is_empty() {
            return Ok(buf.len());
        }

        match self.level {
            Level::WARN => display::warn(raw),
            Level::ERROR => display::error(raw),
            Level::INFO => route_info(raw),
            _ => display::info(raw),
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Route INFO messages to ok() or info() based on content,
/// and emit structured detail lines for rich events.
fn route_info(raw: &str) {
    // ── Success events → green ✓ ──────────────────────────────────────────
    if raw.starts_with("EchoTrap listening on") {
        display::ok(raw);
        return;
    }
    if raw.starts_with("Migration complete") {
        display::ok(raw);
        return;
    }
    if raw.starts_with("Dashboard listening on") {
        display::ok(raw);
        return;
    }
    if raw.starts_with("[DECOY] Decoy listener active") {
        display::ok(raw);
        return;
    }
    if raw.starts_with("EchoTrap shut down") {
        display::ok(raw);
        return;
    }

    // ── Migration requested — emit with detail ────────────────────────────
    if raw.starts_with("Migration requested") {
        display::info(raw);
        return;
    }

    // ── Accepted connection — parse peer and persona ──────────────────────
    if raw.starts_with("Accepted connection from") {
        // "Accepted connection from 127.0.0.1:50844 [persona: ssh]"
        display::info(raw);
        return;
    }

    // ── Decoy scanner probe ───────────────────────────────────────────────
    if raw.starts_with("[DECOY] Scanner") {
        display::info(raw);
        return;
    }

    // ── Shutdown sequence ─────────────────────────────────────────────────
    if raw.starts_with("Ctrl-C received")
        || raw.starts_with("Waiting up to")
        || raw.starts_with("Shutdown signal received")
        || raw.starts_with("Listener on")
        || raw.starts_with("[DECOY] Decoy on")
        || raw.starts_with("Spawning listener")
        || raw.starts_with("Migration request ignored")
        || raw.starts_with("Connection closed")
        || raw.starts_with("[SSH]")
        || raw.starts_with("[HTTP]")
        || raw.starts_with("[Redis]")
        || raw.starts_with("[Raw]")
    {
        display::info(raw);
        return;
    }

    display::info(raw);
}

struct DisplayMakeWriter;

impl<'a> MakeWriter<'a> for DisplayMakeWriter {
    type Writer = DisplayWriter;

    fn make_writer(&'a self) -> Self::Writer {
        DisplayWriter { level: Level::INFO }
    }

    fn make_writer_for(&'a self, meta: &tracing::Metadata<'_>) -> Self::Writer {
        DisplayWriter {
            level: *meta.level(),
        }
    }
}

pub fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(DisplayMakeWriter)
        .with_target(false)
        .with_level(false)
        .with_line_number(false)
        .without_time()
        .init();
}
