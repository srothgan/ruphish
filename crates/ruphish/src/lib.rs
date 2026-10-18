//! Native application composition and process lifecycle.

mod cli;
mod config;
mod logging;
mod runtime;

/// Execute informational CLI commands or run the configured application.
pub fn entry() -> std::process::ExitCode {
    use std::process::ExitCode;
    let arguments = match cli::parse() {
        Ok(Some(arguments)) => arguments,
        Ok(None) => return ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let result = (|| {
        let config = config::load(&arguments.config)?;
        config.validate(arguments.mode)?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        let result = runtime.block_on(runtime::run(arguments, config));
        runtime.shutdown_timeout(std::time::Duration::from_secs(2));
        result
    })();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Ruphish startup/runtime error: {error}");
            ExitCode::FAILURE
        }
    }
}
