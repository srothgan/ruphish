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
        loop {
            let now = time::OffsetDateTime::now_utc().unix_timestamp();
            tokio::select! {
                biased;
                _=cancel.cancelled()=>return Ok(()),
                result=self.domains.claim(now)=>{
                    if let Some(check)=result.map_err(|_|std::io::Error::other("Domain verification storage unavailable"))? {
                        let observation=tokio::select!{biased;_=cancel.cancelled()=>return Ok(()),v=self.verifier.verify(&check)=>v};
                        self.domains.finish(&check,observation,time::OffsetDateTime::now_utc().unix_timestamp()).await.map_err(|_|std::io::Error::other("Domain verification storage unavailable"))?;
                        continue;
                    }
                }
            }
            tokio::select! {_=cancel.cancelled()=>return Ok(()),_=tokio::time::sleep(Duration::from_secs(1))=>{}}
        }
    }
}
