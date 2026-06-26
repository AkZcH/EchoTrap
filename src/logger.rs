// src/logger.rs
use crate::display;
use std::io::Write;
use std::path::Path;
use tracing::Level;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

// ── Terminal layer ────────────────────────────────────────────────────────────

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

fn route_info(raw: &str) {
    if raw.starts_with("EchoTrap listening on")
        || raw.starts_with("Migration complete")
        || raw.starts_with("Dashboard listening on")
        || raw.starts_with("[DECOY] Decoy listener active")
        || raw.starts_with("EchoTrap shut down")
    {
        display::ok(raw);
    } else {
        display::info(raw);
    }
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

// ── Public init ───────────────────────────────────────────────────────────────

/// Initialise tracing with two layers:
///   1. Terminal — HERALD-styled display output
///   2. File — NDJSON for SIEM ingestion (non-blocking background writer)
///
/// Returns a `WorkerGuard` that must be kept alive in `main` for the process
/// lifetime. Dropping it flushes and closes the file writer.
pub fn init_tracing(log_path: &str) -> WorkerGuard {
    let path = Path::new(log_path);
    let dir = path.parent().unwrap_or(Path::new("."));
    let filename = path
        .file_name()
        .unwrap_or(std::ffi::OsStr::new("echotrap.log"))
        .to_string_lossy()
        .into_owned();

    let file_appender = tracing_appender::rolling::never(dir, &filename);
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    // Terminal layer — styled HERALD output.
    let terminal_layer = tracing_subscriber::fmt::layer()
        .with_writer(DisplayMakeWriter)
        .with_target(false)
        .with_level(false)
        .with_line_number(false)
        .without_time()
        // Box to erase the concrete type so both layers can coexist.
        .boxed();

    // JSON file layer — NDJSON, one object per line.
    // {"timestamp":"...","level":"INFO","fields":{"message":"..."}}
    let json_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_target(false)
        .with_level(true)
        .with_line_number(false)
        .json()
        .flatten_event(true)
        .with_current_span(false)
        .with_span_list(false)
        .boxed();

    tracing_subscriber::registry()
        .with(filter)
        .with(terminal_layer)
        .with(json_layer)
        .init();

    guard
}
