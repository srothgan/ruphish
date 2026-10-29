//! Admission for owned content parsing and rendering outside async workers.
use crate::resources::Error;
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Shared rendering capacity, retained by computation even if its caller is cancelled.
#[derive(Clone)]
pub struct ContentWork {
    capacity: Arc<Semaphore>,
}

impl ContentWork {
    /// Use the installation's existing rendering admission limit.
    pub fn new(capacity: Arc<Semaphore>) -> Self {
        Self { capacity }
    }

    /// Admit one owned parser/composer operation; cancellation cannot preempt its work.
    pub async fn run<T: Send + 'static>(
        &self,
        work: impl FnOnce() -> Result<T, Error> + Send + 'static,
    ) -> Result<T, Error> {
        let permit = self
            .capacity
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| Error::Internal)?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            work()
        })
        .await
        .map_err(|_| Error::Internal)?
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "Controlled worker setup and assertions")]
mod tests {
    use super::*;
    use std::{future::Future, task::Poll};

    struct Release(Option<std::sync::mpsc::Sender<()>>);
    impl Drop for Release {
        fn drop(&mut self) {
            if let Some(sender) = self.0.take() {
                let _ = sender.send(());
            }
        }
    }

    #[test]
    fn cancelled_content_keeps_capacity_until_computation_ends_without_blocking_async_work() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        runtime.block_on(async {
            let capacity = Arc::new(Semaphore::new(1));
            let work = ContentWork::new(capacity.clone());
            let (release, blocked) = std::sync::mpsc::channel();
            let mut release = Release(Some(release));
            let (started, start) = tokio::sync::oneshot::channel();
            let first = work.clone();
            let request = tokio::spawn(async move {
                first
                    .run(move || {
                        let _ = started.send(());
                        blocked
                            .recv_timeout(std::time::Duration::from_secs(20))
                            .map_err(|_| Error::Internal)?;
                        Ok(())
                    })
                    .await
            });
            start.await.unwrap();
            // The sole async worker can complete unrelated work while content is blocked.
            assert_eq!(tokio::spawn(async { 42 }).await.unwrap(), 42);
            request.abort();
            assert!(request.await.unwrap_err().is_cancelled());
            assert_eq!(capacity.available_permits(), 0);
            let mut next = Box::pin(work.run(|| Ok(7)));
            let pending =
                std::future::poll_fn(|cx| Poll::Ready(next.as_mut().poll(cx).is_pending())).await;
            assert!(pending, "cancellation must not admit a second computation");
            release.0.take().unwrap().send(()).unwrap();
            assert_eq!(next.await.unwrap(), 7);
            assert_eq!(capacity.available_permits(), 1);
            assert!(matches!(
                work.run::<()>(|| Err(Error::Changed)).await,
                Err(Error::Changed)
            ));
            capacity.close();
            assert!(matches!(work.run(|| Ok(())).await, Err(Error::Internal)));
        });
    }
}
