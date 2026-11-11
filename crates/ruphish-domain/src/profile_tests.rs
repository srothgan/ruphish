//! Persisted sender diagnostics and campaign readiness, independent of transport.
use serde::{Deserialize, Serialize};

/// Bound stored administrator test recipients consistently across providers.
pub const MAX_RECIPIENT_BYTES: usize = 320;

/// Explicit operation: a connection check never submits message content.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Connect and authenticate.
    Connection,
    /// Submit the fixed profile-test message.
    Email,
}

/// Durable execution state. Unknown submissions must never be retried automatically.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// No content has been submitted.
    Running,
    /// Content may have been transmitted.
    Submitting,
    /// Connection and authentication succeeded.
    Passed,
    /// SMTP explicitly accepted the message.
    Accepted,
    /// Definite failure before acceptance or an explicit SMTP rejection.
    Failed,
    /// Execution ended before submission.
    Interrupted,
    /// Acceptance cannot be established or excluded.
    Unknown,
}

impl Outcome {
    /// True while an owner may still complete the attempt.
    pub fn active(self) -> bool {
        matches!(self, Self::Running | Self::Submitting)
    }
    /// Whether this result revokes previously accepted submission evidence.
    pub fn revokes(self) -> bool {
        matches!(self, Self::Failed | Self::Interrupted | Self::Unknown)
    }
}

/// A safe operational record, without credentials or peer-provided error strings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attempt {
    /// Stable request identity, also used to reconcile a lost response.
    pub id: String,
    /// Configuration generation actually tested.
    pub generation: i64,
    /// Connection or fixed-content email.
    pub kind: Kind,
    /// Operator identity, retained when the account is deleted.
    pub operator: i64,
    /// Chosen administrator test recipient, absent for connection checks.
    pub email: Option<String>,
    /// Start time in UTC Unix seconds.
    pub started_at: i64,
    /// Completion time, absent while active.
    pub finished_at: Option<i64>,
    /// Explicit execution result.
    pub outcome: Outcome,
    /// Application-owned diagnostic code.
    pub code: String,
    /// Optional numeric SMTP rejection code.
    pub smtp_code: Option<u16>,
}

/// Current profile evidence and the latest ten completed attempts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Summary {
    /// Current sending configuration generation.
    pub generation: i64,
    /// Whether company policy requires accepted profile-test evidence.
    pub required: bool,
    /// Current evidence qualifies under the company policy.
    pub eligible: bool,
    /// Current attempt, including its stable identity for reconciliation.
    pub active: Option<Attempt>,
    /// Latest connection-only result, including historical configurations.
    pub connection: Option<Attempt>,
    /// Latest fixed-content email result, including historical configurations.
    pub email: Option<Attempt>,
    /// Latest completed attempts, newest first.
    pub history: Vec<Attempt>,
}

/// Compare ordered current-generation evidence. A later diagnostic failure revokes acceptance.
pub fn qualifies<'a>(attempts: impl IntoIterator<Item = &'a Attempt>, generation: i64) -> bool {
    for attempt in attempts {
        if attempt.generation != generation {
            continue;
        }
        if attempt.outcome.revokes() {
            return false;
        }
        if attempt.kind == Kind::Email && attempt.outcome == Outcome::Accepted {
            return true;
        }
    }
    false
}
