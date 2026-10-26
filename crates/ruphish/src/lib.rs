//! Native application composition and process lifecycle.

mod campaign_worker;
mod cli;
mod config;
mod diagnostics;
mod domain_worker;
mod logging;
mod runtime;
mod worker_backoff;

/// Execute informational CLI commands or run the configured application.
pub fn entry() -> std::process::ExitCode {
    use std::process::ExitCode;
    let arguments = match cli::parse() {
        Ok(Some(arguments)) => arguments,
        Ok(None) => return ExitCode::SUCCESS,
        Err(error) => {
            // Clap's Display echoes untrusted argument values, which may be secrets.
            eprintln!(
                "Invalid command line ({:?}); use --help for supported options",
                error.kind()
            );
            return ExitCode::FAILURE;
        }
    };
    let result = (|| {
        let config = config::load(&arguments.config)?;
        config.validate(arguments.mode)?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|error| diagnostics::context("Initialize async runtime", error))?;
        let result = runtime.block_on(runtime::run(arguments, config));
        runtime.shutdown_timeout(std::time::Duration::from_secs(2));
        result
    })();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            // Runtime errors can contain database trigger text or peer diagnostics.
            // Only configuration errors and contextual diagnostics are safe to display.
            if let Some(config::ConfigError::Json { line, column }) =
                error.downcast_ref::<config::ConfigError>()
            {
                eprintln!("Invalid configuration JSON at line {line}, column {column}");
            } else if let Some(config::ConfigError::Io(error)) =
                error.downcast_ref::<config::ConfigError>()
            {
                eprintln!(
                    "Could not read configuration ({:?}); check its path and permissions",
                    error.kind()
                );
            } else if let Some(config::ConfigError::Invalid(message)) =
                error.downcast_ref::<config::ConfigError>()
            {
                eprintln!("Invalid configuration: {message}");
            } else if let Some(error) = error.downcast_ref::<diagnostics::Failure>() {
                eprintln!("Ruphish startup/runtime failed: {error}");
            } else {
                eprintln!(
                    "Ruphish startup/runtime failed; check configuration, asset integrity, TLS files and database availability. Untrusted error details are suppressed."
                );
            }
            ExitCode::FAILURE
        }
    }
}
