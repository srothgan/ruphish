use crate::config::{ConfigError, Logging};
use std::io::{self, Write};
use tracing_subscriber::prelude::*;
use tracing_subscriber::{
    filter::LevelFilter,
    fmt::writer::{BoxMakeWriter, MakeWriterExt},
};

pub(crate) fn level(value: &str) -> Result<LevelFilter, ConfigError> {
    match value.to_ascii_lowercase().as_str() {
        "" | "info" => Ok(LevelFilter::INFO),
        "trace" => Ok(LevelFilter::TRACE),
        "debug" => Ok(LevelFilter::DEBUG),
        "warn" | "warning" => Ok(LevelFilter::WARN),
        "error" => Ok(LevelFilter::ERROR),
        // Tracing's ERROR is the severity used for fatal application failures.
        "fatal" | "panic" => Ok(LevelFilter::ERROR),
        _ => Err(ConfigError::Invalid("Invalid logging.level".into())),
    }
}

pub(crate) fn setup(
    config: &Logging,
) -> Result<
    Option<tracing_appender::non_blocking::WorkerGuard>,
    Box<dyn std::error::Error + Send + Sync>,
> {
    let filter = level(&config.level)?;
    let (writer, guard) = if config.filename.as_os_str().is_empty() {
        (BoxMakeWriter::new(std::io::stderr), None)
    } else {
        let file =
            ruphish_adapters::rotating_log::Writer::open(&config.filename, config.rotation())?;
        let (writer, guard) = tracing_appender::non_blocking::NonBlockingBuilder::default()
            .buffered_lines_limit(4096)
            .lossy(false)
            .finish(ReportedFile {
                inner: file,
                failed: false,
            });
        (BoxMakeWriter::new(std::io::stderr.and(writer)), Some(guard))
    };
    let targets = tracing_subscriber::filter::Targets::new()
        // Dependencies can emit protocol bodies or peer diagnostics even at WARN.
        // Operational failures are translated at our typed application boundaries.
        .with_default(LevelFilter::OFF)
        .with_target("ruphish", filter);
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_target(false)
                .with_writer(writer)
                .with_filter(targets),
        )
        .try_init()?;
    Ok(guard)
}

// The worker otherwise suppresses I/O errors. Stderr already receives each event; report only
// transitions, never raw filesystem error text, and retry file output on the next record.
struct ReportedFile {
    inner: ruphish_adapters::rotating_log::Writer,
    failed: bool,
}

impl Write for ReportedFile {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        match self.inner.write_all(bytes) {
            Ok(()) if self.failed => {
                let _ = writeln!(
                    io::stderr(),
                    "Application file logging resumed; stderr logging remained active."
                );
                self.failed = false;
            }
            Err(error) => {
                if !self.failed {
                    let _ = writeln!(
                        io::stderr(),
                        "Application file logging unavailable ({:?}); stderr logging remains active. Check log directory access, free space and archive files.",
                        error.kind()
                    );
                }
                self.failed = true;
            }
            Ok(()) => {}
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
