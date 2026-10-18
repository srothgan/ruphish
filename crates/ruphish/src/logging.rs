use crate::config::{ConfigError, Logging};
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
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&config.filename)?;
        let (writer, guard) = tracing_appender::non_blocking::NonBlockingBuilder::default()
            .lossy(false)
            .finish(file);
        (BoxMakeWriter::new(std::io::stderr.and(writer)), Some(guard))
    };
    let targets = tracing_subscriber::filter::Targets::new()
        .with_default(filter.min(LevelFilter::WARN))
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
