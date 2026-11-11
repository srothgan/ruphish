//! Bounded retry pacing for durable background work after storage failures.
use std::time::Duration;
use tokio::time::Instant;

pub(crate) struct Backoff {
    failures: u32,
    next: Instant,
}
impl Backoff {
    pub(crate) fn new() -> Self {
        Self {
            failures: 0,
            next: Instant::now(),
        }
    }
    pub(crate) fn ready(&self) -> bool {
        Instant::now() >= self.next
    }
    pub(crate) fn failed(&mut self) -> Duration {
        let delay = Duration::from_secs((1_u64 << self.failures.min(5)).min(30));
        self.failures = self.failures.saturating_add(1);
        self.next = Instant::now() + delay;
        delay
    }
    pub(crate) fn succeeded(&mut self) {
        self.failures = 0;
        self.next = Instant::now();
    }
}
