//! Installation policy. Durations are saved even while verification is disabled.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// Installation-wide policy with bounded verification scheduling.
pub struct Policy {
    /// Whether verification is required and checks may run.
    pub verification_enabled: bool,
    /// Whether successful evidence has a time limit.
    pub verification_expires: bool,
    /// Successful evidence lifetime in seconds.
    pub verification_validity_seconds: i64,
    /// Automatic recheck interval in seconds.
    pub verification_recheck_seconds: i64,
    /// Whether mailbox network operations are authorized.
    pub reporting_mailboxes_enabled: bool,
    /// Whether webhook enqueueing and delivery are authorized.
    pub webhooks_enabled: bool,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            verification_enabled: false,
            verification_expires: true,
            verification_validity_seconds: 86400,
            verification_recheck_seconds: 21600,
            reporting_mailboxes_enabled: false,
            webhooks_enabled: false,
        }
    }
}

impl Policy {
    /// Reject unsupported durations and insufficient retry margin.
    pub fn validate(&self) -> Result<(), &'static str> {
        self.validation().finish().map_err(|e| e.message())
    }

    /// Independent policy issues, including the cross-field retry margin.
    pub fn validation(&self) -> crate::validation::Report {
        use crate::validation::{Code, Report};
        let mut report = Report::default();
        let validity = (3600..=365 * 86400).contains(&self.verification_validity_seconds);
        let recheck = (300..=7 * 86400).contains(&self.verification_recheck_seconds);
        if !validity {
            report.reject(
                "verification_validity_seconds",
                Code::ValidityRange,
                "settings_invalid_validity",
            );
        }
        if !recheck {
            report.reject(
                "verification_recheck_seconds",
                Code::RecheckRange,
                "settings_invalid_recheck",
            );
        }
        if validity
            && recheck
            && self.verification_expires
            && self.verification_recheck_seconds + self.jitter_bound()
                > self.verification_validity_seconds / 2
        {
            report.reject(
                "verification_recheck_seconds",
                Code::RecheckMargin,
                "settings_recheck_margin_required",
            );
        }
        report
    }

    fn jitter_bound(&self) -> i64 {
        (self.verification_recheck_seconds / 10).min(300)
    }

    /// Stable bounded jitter around the configured recheck interval.
    pub fn refresh_delay(&self, domain: i64) -> i64 {
        let bound = self.jitter_bound();
        self.verification_recheck_seconds + domain.wrapping_mul(97).rem_euclid(2 * bound + 1)
            - bound
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
/// Safe effective feature flags for authenticated users.
pub struct Features {
    /// Whether verification is required and checks may run.
    pub verification_enabled: bool,
    /// Whether mailbox network operations are authorized.
    pub reporting_mailboxes_enabled: bool,
    /// Whether webhook enqueueing and delivery are authorized.
    pub webhooks_enabled: bool,
}

impl From<&Policy> for Features {
    fn from(policy: &Policy) -> Self {
        Self {
            verification_enabled: policy.verification_enabled,
            reporting_mailboxes_enabled: policy.reporting_mailboxes_enabled,
            webhooks_enabled: policy.webhooks_enabled,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// Persisted policy and optimistic write revision.
pub struct Settings {
    /// Optimistic concurrency revision.
    pub revision: i64,
    /// Complete installation policy.
    pub policy: Policy,
    /// Last policy change in UTC Unix seconds.
    pub updated_at: i64,
    /// Administrator responsible for the last policy change.
    pub updated_by: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// Expiring, actor-bound review of the active campaign set.
pub struct ClosurePreview {
    /// Opaque actor-bound confirmation identity.
    pub token: String,
    /// Confirmation deadline in UTC Unix seconds.
    pub expires_at: i64,
    /// Number of affected launched campaigns across owners.
    pub campaigns: i64,
    /// Number of unsent deliveries at review time.
    pub pending_deliveries: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// Durable result returned again when a closure request is replayed.
pub struct ClosureResult {
    /// Number of affected launched campaigns across owners.
    pub campaigns: i64,
    /// Number of unsent deliveries cancelled by the operation.
    pub cancelled_deliveries: i64,
    /// Completion time in UTC Unix seconds.
    pub completed_at: i64,
}
