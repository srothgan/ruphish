//! Actionable startup context without formatting untrusted error text.
use std::{error::Error, io};

type BoxError = Box<dyn Error + Send + Sync>;

#[derive(Debug, thiserror::Error)]
#[error("{operation}: {reason}")]
pub(crate) struct Failure {
    operation: &'static str,
    reason: String,
    #[source]
    source: BoxError,
}

pub(crate) fn context(operation: &'static str, error: impl Into<BoxError>) -> BoxError {
    let source = error.into();
    let mut current: Option<&(dyn Error + 'static)> = Some(source.as_ref());
    let mut reason =
        "operation failed; check the corresponding configuration and prerequisites".into();
    while let Some(error) = current {
        if let Some(error) = error.downcast_ref::<io::Error>() {
            reason = format!("filesystem or network error ({:?})", error.kind());
            break;
        }
        if let Some(error) = error.downcast_ref::<ruphish_adapters::database::Error>() {
            use ruphish_adapters::database::Error;
            reason = match error {
                Error::Invalid(_) => "invalid database configuration or schema".into(),
                // All other storage variants contain only closed categories, not peer text.
                _ => error.to_string(),
            };
            break;
        }
        if matches!(
            error.downcast_ref::<ruphish_adapters::tls::CertificateError>(),
            Some(ruphish_adapters::tls::CertificateError::Incomplete)
        ) {
            reason = "certificate and key must be distinct files and either both exist or both be absent; existing files were kept".into();
            break;
        }
        current = error.source();
    }
    Box::new(Failure {
        operation,
        reason,
        source,
    })
}
