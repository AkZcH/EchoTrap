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
        match self.level {
            Level::WARN => display::warn(raw),
            Level::ERROR => display::error(raw),
            Level::INFO => {
                if raw.starts_with("EchoTrap listening")
                    || raw.starts_with("Migration complete")
                    || raw.starts_with("Dashboard listening")
                    || raw.starts_with("[DECOY] Decoy listener active")
                    || raw.starts_with("EchoTrap shut down")
                {
                    display::ok(raw);
                } else {
                    display::info(raw);
                }
            }
            _ => display::info(raw),
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
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
