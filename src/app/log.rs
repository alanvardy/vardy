use std::io::{self, Write};
use tracing_subscriber::{EnvFilter, fmt};

/// Writer that silently drops `BrokenPipe` errors on stderr instead of
/// panicking. On Unix, stderr is often a pipe (journald, Fly.io capture,
/// or a terminal that exits); when the downstream end closes, write()
/// returns Err(BrokenPipe) and the default writer panics on it.
struct StderrWriter;

impl Write for StderrWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match io::stderr().write(buf) {
            Ok(n) => Ok(n),
            Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(buf.len()),
            Err(e) => Err(e),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match io::stderr().flush() {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
            Err(e) => Err(e),
        }
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for StderrWriter {
    type Writer = Self;

    fn make_writer(&self) -> Self::Writer {
        StderrWriter
    }
}

// Emit one structured JSON log line per event so Fly.io's stdout capture can
// forward request logs to downstream aggregators such as Loki/Grafana.
pub fn init() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,tower_http=info"));

    fmt()
        .json()
        .flatten_event(true)
        .with_current_span(false)
        .with_env_filter(filter)
        .with_writer(StderrWriter)
        .init();
}
