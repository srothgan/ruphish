//! Operational readiness, independent of HTTP and concrete database providers.

use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};
use std::time::Duration;
use tokio::sync::Semaphore;

/// Read-only access to the application's actual storage, including pool acquisition.
#[async_trait::async_trait]
pub trait Storage: Send + Sync {
    /// Check the application schema without reading credentials or participant data.
    async fn check(&self) -> Result<(), crate::storage::Error>;
}

const STARTING: u8 = 0;
const SERVING: u8 = 1;
const DRAINING: u8 = 2;
const CHECK_TIMEOUT: Duration = Duration::from_secs(1);

/// One readiness owner shared by the supervisor and all enabled listeners.
#[derive(Clone)]
pub struct Service(Arc<State>);

struct State {
    lifecycle: AtomicU8,
    capacity: Semaphore,
    storage: Arc<dyn Storage>,
}

impl Service {
    /// Start unavailable until the supervisor has started every required task.
    pub fn new(storage: Arc<dyn Storage>) -> Self {
        Self(Arc::new(State {
            lifecycle: AtomicU8::new(STARTING),
            capacity: Semaphore::new(1),
            storage,
        }))
    }

    /// Called by the supervisor after initialization and required task startup.
    /// A shutdown decision cannot be reversed by a late startup notification.
    pub fn started(&self) {
        let _ = self.0.lifecycle.compare_exchange(
            STARTING,
            SERVING,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
    }

    /// Called before cancelling tasks or draining listeners, on every exit path.
    pub fn draining(&self) {
        self.0.lifecycle.store(DRAINING, Ordering::SeqCst);
    }

    /// Bound probe work without queueing behind another probe or caching success.
    pub async fn ready(&self) -> bool {
        if self.0.lifecycle.load(Ordering::SeqCst) != SERVING {
            return false;
        }
        let Ok(_permit) = self.0.capacity.try_acquire() else {
            return false;
        };
        let available = matches!(
            tokio::time::timeout(CHECK_TIMEOUT, self.0.storage.check()).await,
            Ok(Ok(()))
        );
        available && self.0.lifecycle.load(Ordering::SeqCst) == SERVING
    }
}
