//! Independent state, diagnostic and retry expectations for SMTP outcomes.
#![allow(clippy::unwrap_used, reason = "Domain outcome assertions")]
use ruphish_domain::campaign::*;

#[test]
fn actionable_setup_failures_preserve_scope_and_retry_semantics() {
    for (stage, kind, expected_code, expected_state) in [
        (
            Stage::Configuration,
            FailureKind::Blocked,
            "smtp_configuration_invalid",
            DeliveryState::Blocked,
        ),
        (
            Stage::DestinationPolicy,
            FailureKind::Blocked,
            "smtp_destination_denied",
            DeliveryState::Blocked,
        ),
        (
            Stage::Dns,
            FailureKind::Retryable,
            "smtp_dns_unavailable",
            DeliveryState::Retry,
        ),
        (
            Stage::Connect,
            FailureKind::Retryable,
            "smtp_connect_unavailable",
            DeliveryState::Retry,
        ),
        (
            Stage::Tls,
            FailureKind::Blocked,
            "smtp_tls_failed",
            DeliveryState::Blocked,
        ),
        (
            Stage::Authentication,
            FailureKind::Blocked,
            "smtp_authentication_failed",
            DeliveryState::Blocked,
        ),
        (
            Stage::Authentication,
            FailureKind::Retryable,
            "smtp_authentication_failed",
            DeliveryState::Retry,
        ),
    ] {
        let result = delivery_update(
            DeliveryState::Preparing,
            false,
            0,
            1000,
            &Err(DeliveryFailure::new(kind, stage, None)),
            0,
        )
        .unwrap();
        assert_eq!(result.state, expected_state);
        assert_eq!(result.code, expected_code);
        assert!(result.cooldown, "setup failures affect this profile");
        assert_eq!(result.count, u32::from(kind == FailureKind::Retryable));
        assert_eq!(
            result.next,
            if kind == FailureKind::Retryable {
                1120
            } else {
                1000
            }
        );
    }
}

#[test]
fn recipient_rejections_uncertainty_and_exhaustion_do_not_become_profile_blocks() {
    for (kind, smtp_code, code, state) in [
        (
            FailureKind::Retryable,
            Some(451),
            "smtp_temporary_rejection",
            DeliveryState::Retry,
        ),
        (
            FailureKind::Permanent,
            Some(550),
            "smtp_permanent_rejection",
            DeliveryState::Failed,
        ),
        (
            FailureKind::Unknown,
            None,
            "delivery_outcome_unknown",
            DeliveryState::Unknown,
        ),
    ] {
        let result = delivery_update(
            DeliveryState::Submitting,
            false,
            0,
            1000,
            &Err(DeliveryFailure::new(kind, Stage::Acceptance, smtp_code)),
            0,
        )
        .unwrap();
        assert_eq!(result.state, state);
        assert_eq!(result.code, code);
        assert_eq!(result.smtp_code, smtp_code);
        assert!(!result.cooldown);
    }
    let exhausted = delivery_update(
        DeliveryState::Preparing,
        false,
        8,
        1000,
        &Err(DeliveryFailure::new(
            FailureKind::Retryable,
            Stage::Dns,
            None,
        )),
        0,
    )
    .unwrap();
    assert_eq!(exhausted.state, DeliveryState::Failed);
    assert_eq!(exhausted.code, "delivery_retry_exhausted");
}
