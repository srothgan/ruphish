//! Supervised bounded campaign execution; the database owns runnable work.
use ruphish_adapters::smtp::Smtp;
use ruphish_application::{
    campaign::{Claim, DeliveryFailure, FailureKind, Queue, Stage},
    delivery::{Sender, Transport},
    resources::Error,
};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

pub(crate) struct Worker {
    pub queue: Arc<dyn Queue>,
    pub smtp: Arc<Smtp>,
}
fn now() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}
async fn execute(queue: Arc<dyn Queue>, smtp: Arc<Smtp>, claim: Arc<Claim>) -> Result<(), Error> {
    let message = claim.message.clone();
    let composer = smtp.clone();
    let prepared = tokio::task::spawn_blocking(move || composer.compose(&message))
        .await
        .map_err(|_| Error::Internal)?;
    let outcome = match prepared {
        Err(_) => Err(DeliveryFailure::new(
            FailureKind::Permanent,
            Stage::Content,
            None,
        )),
        Ok(mail) => match smtp.prepare(&claim.message.profile, mail).await {
            Err(e) if e.stage == Stage::Capacity => {
                queue.release(&claim, now()).await?;
                return Ok(());
            }
            Err(e) => Err(e),
            Ok(pending) => {
                if !queue.arm(&claim, now()).await? {
                    queue.release(&claim, now()).await?;
                    return Ok(());
                }
                pending.submit().await
            }
        },
    };
    // A transient database failure after acceptance retries only the durable write.
    for attempt in 0..4 {
        match queue.finish(&claim, outcome.clone(), now()).await {
            Ok(()) => return Ok(()),
            Err(e) if attempt == 3 => return Err(e),
            Err(_) => tokio::time::sleep(Duration::from_millis(100 * (1 << attempt))).await,
        }
    }
    Err(Error::Internal)
}
impl Worker {
    pub async fn run(self, stop: CancellationToken) -> Result<(), std::io::Error> {
        let mut tasks: JoinSet<(i64, Result<(), Error>)> = JoinSet::new();
        let mut active = HashMap::<i64, Arc<Claim>>::new();
        let mut backoff = crate::worker_backoff::Backoff::new();
        let mut tick = tokio::time::interval(Duration::from_millis(500));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let result = async {
            loop {
                tokio::select! {
                    biased;
                    _ = stop.cancelled() => return Ok::<_, std::io::Error>(()),
                    joined = tasks.join_next(), if !tasks.is_empty() => {
                        let (id, outcome) = joined
                            .ok_or_else(|| std::io::Error::other("Campaign worker lost task"))?
                            .map_err(std::io::Error::other)?;
                        active.remove(&id);
                        if outcome.is_err() {
                            // The durable lease retains ambiguous submission state. Never rerun SMTP here.
                            let delay = backoff.failed();
                            tracing::warn!(retry_seconds = delay.as_secs(), "campaign_outcome_storage_unavailable");
                        }
                    },
                    _ = tick.tick() => {
                        if !backoff.ready() {
                            continue;
                        }
                        while tasks.len() < 4 && !stop.is_cancelled() {
                            let claim = match self.queue.claim(now()).await {
                                Ok(claim) => {
                                    backoff.succeeded();
                                    claim
                                },
                                Err(_) => {
                                    let delay = backoff.failed();
                                    tracing::warn!(retry_seconds = delay.as_secs(), "campaign_queue_storage_unavailable");
                                    break;
                                }
                            };
                            let Some(claim) = claim else { break };
                            let id = claim.result_id;
                            let claim = Arc::new(claim);
                            active.insert(id, claim.clone());
                            let queue = self.queue.clone();
                            let smtp = self.smtp.clone();
                            tasks.spawn(async move { (id, execute(queue, smtp, claim).await) });
                        }
                    }
                }
            }
        }.await;
        let drained = tokio::time::timeout(Duration::from_secs(7), async {
            while let Some(task) = tasks.join_next().await {
                let (id, outcome) = task.map_err(std::io::Error::other)?;
                active.remove(&id);
                outcome.map_err(std::io::Error::other)?;
            }
            Ok::<_, std::io::Error>(())
        })
        .await;
        tasks.abort_all();
        while tasks.join_next().await.is_some() {}
        for claim in active.values() {
            if !self
                .queue
                .release(claim, now())
                .await
                .map_err(std::io::Error::other)?
            {
                self.queue
                    .finish(
                        claim,
                        Err(DeliveryFailure::new(
                            FailureKind::Unknown,
                            Stage::Acceptance,
                            None,
                        )),
                        now(),
                    )
                    .await
                    .map_err(std::io::Error::other)?;
            }
        }
        result?;
        if let Ok(result) = drained {
            result?;
        }
        Ok(())
    }
}
