//! Durable verification is independent of the SMTP worker and its retry budget.
use ruphish_application::domains::{Repository, Verifier};
use std::{sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;

pub(crate) struct Worker {
    pub(crate) domains: Arc<dyn Repository>,
    pub(crate) verifier: Arc<dyn Verifier>,
}
impl Worker {
    pub(crate) async fn run(self, cancel: CancellationToken) -> Result<(), std::io::Error> {
        let mut backoff = crate::worker_backoff::Backoff::new();
        loop {
            let now = ruphish_application::datetime::now().as_second();
            let result = tokio::select! {
                biased;
                _=cancel.cancelled()=>return Ok(()),
                result=self.domains.claim(now)=>result,
            };
            let result = match result {
                Ok(Some(check)) => {
                    let observation = tokio::select! {biased;_=cancel.cancelled()=>return Ok(()),v=self.verifier.verify(&check)=>v};
                    let completed = tokio::select! {biased;_=cancel.cancelled()=>return Ok(()),v=self.domains.finish(&check,observation,ruphish_application::datetime::now().as_second())=>v};
                    if completed.is_ok() {
                        backoff.succeeded();
                        continue;
                    }
                    completed
                }
                Ok(None) => Ok(()),
                Err(error) => Err(error),
            };
            let delay = if result.is_err() {
                let delay = backoff.failed();
                tracing::warn!(
                    retry_seconds = delay.as_secs(),
                    "domain_verification_storage_unavailable"
                );
                delay
            } else {
                backoff.succeeded();
                Duration::from_secs(1)
            };
            tokio::select! {_=cancel.cancelled()=>return Ok(()),_=tokio::time::sleep(delay)=>{}}
        }
    }
}
