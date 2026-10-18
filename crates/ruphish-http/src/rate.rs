//! Bounded login admission; governor owns token-bucket timing and accounting.
use governor::{Quota, RateLimiter, clock::Clock, state::keyed::DefaultKeyedStateStore};
use std::{net::IpAddr, num::NonZeroU32};

pub(crate) struct LoginLimiter<C: Clock> {
    buckets: RateLimiter<
        IpAddr,
        DefaultKeyedStateStore<IpAddr>,
        C,
        governor::middleware::NoOpMiddleware<C::Instant>,
    >,
}
impl<C: Clock> LoginLimiter<C> {
    pub(crate) fn new(clock: C) -> Self {
        Self {
            buckets: RateLimiter::new(
                Quota::per_minute(NonZeroU32::new(5).unwrap_or(NonZeroU32::MIN)),
                DefaultKeyedStateStore::default(),
                clock,
            ),
        }
    }
    pub(crate) fn admit(&self, peer: IpAddr) -> bool {
        self.buckets.retain_recent();
        self.buckets.len() < 10_000 && self.buckets.check_key(&peer).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use governor::clock::FakeRelativeClock;
    #[test]
    fn expired_login_buckets_are_reclaimed_without_resetting_active_limits() {
        let clock = FakeRelativeClock::default();
        let limiter = LoginLimiter::new(clock.clone());
        let alice = IpAddr::from([127, 0, 0, 1]);
        let bob = IpAddr::from([127, 0, 0, 2]);
        for _ in 0..5 {
            assert!(limiter.admit(alice));
        }
        assert!(!limiter.admit(alice));
        assert!(limiter.admit(bob));
        assert!(
            !limiter.admit(alice),
            "Cleanup must not grant another burst to an active client"
        );
        clock.advance(std::time::Duration::from_secs(12));
        assert!(limiter.admit(alice));
        assert!(!limiter.admit(alice));
        clock.advance(std::time::Duration::from_secs(120));
        limiter.buckets.retain_recent();
        assert!(limiter.buckets.is_empty());
        for _ in 0..5 {
            assert!(limiter.admit(alice));
        }
        assert!(!limiter.admit(alice));
    }
}
