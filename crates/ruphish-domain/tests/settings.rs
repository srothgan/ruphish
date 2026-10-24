//! Bounded policy scheduling, including expiration and retry headroom.
use ruphish_domain::settings::Policy;

#[test]
fn duration_boundaries_and_retry_headroom_are_enforced() {
    let mut p = Policy::default();
    assert!(p.validate().is_ok());
    for invalid in [i64::MIN, 0, 3599, 31_536_001, i64::MAX] {
        p.verification_validity_seconds = invalid;
        assert!(p.validate().is_err());
    }
    p.verification_validity_seconds = 3600;
    for invalid in [i64::MIN, 0, 299, 604801, i64::MAX] {
        p.verification_recheck_seconds = invalid;
        assert!(p.validate().is_err());
    }
    p.verification_recheck_seconds = 1800;
    assert!(
        p.validate().is_err(),
        "jitter must leave room for retry before expiry"
    );
    p.verification_recheck_seconds = 900;
    assert!(p.validate().is_ok());
    p.verification_expires = false;
    p.verification_recheck_seconds = 604800;
    assert!(
        p.validate().is_ok(),
        "nonexpiring evidence still permits periodic checks"
    );
}

#[test]
fn recheck_jitter_never_exceeds_configured_bounds() {
    for interval in [300, 900, 21600, 604800] {
        let p = Policy {
            verification_recheck_seconds: interval,
            ..Default::default()
        };
        let bound = (interval / 10).min(300);
        for id in [i64::MIN, -1, 0, 1, 10, 9000, i64::MAX] {
            assert!((interval - bound..=interval + bound).contains(&p.refresh_delay(id)));
            assert!(p.refresh_delay(id) > 0);
        }
    }
}
