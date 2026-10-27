//! Canonical campaign lifecycle and transport identities.
/// Recorded campaign lifecycle, including supported imported lifecycle states.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CampaignStatus {
    /// Created but not launched.
    #[serde(rename = "Created")]
    Created,
    /// Waiting for launch.
    #[serde(rename = "Queued")]
    Queued,
    /// Active execution.
    #[serde(rename = "In progress")]
    InProgress,
    /// Imported execution whose messages have been sent.
    #[serde(rename = "Emails Sent")]
    EmailsSent,
    /// Closed execution.
    #[serde(rename = "Completed")]
    Completed,
}
impl CampaignStatus {
    /// Decode the stored identifier; invalid state is not silently defaulted.
    pub fn from_stored(value: &str) -> Option<Self> {
        match value {
            "Created" => Some(Self::Created),
            "Queued" => Some(Self::Queued),
            "In progress" => Some(Self::InProgress),
            "Emails Sent" => Some(Self::EmailsSent),
            "Completed" => Some(Self::Completed),
            _ => None,
        }
    }
    /// Stable value used by storage, presentation and tabular exports.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Created => "Created",
            Self::Queued => "Queued",
            Self::InProgress => "In progress",
            Self::EmailsSent => "Emails Sent",
            Self::Completed => "Completed",
        }
    }
}

/// Transport state reported independently from recipient engagement.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum DeliveryState {
    /// Waiting for a delivery attempt.
    #[serde(rename = "ready")]
    Ready,
    /// Preparing a message.
    #[serde(rename = "preparing")]
    Preparing,
    /// SMTP submission in progress.
    #[serde(rename = "submitting")]
    Submitting,
    /// Waiting for a retry.
    #[serde(rename = "retry")]
    Retry,
    /// SMTP acceptance recorded.
    #[serde(rename = "accepted")]
    Accepted,
    /// Permanent failure.
    #[serde(rename = "failed")]
    Failed,
    /// External delivery outcome is ambiguous.
    #[serde(rename = "unknown")]
    Unknown,
    /// Operator action required.
    #[serde(rename = "blocked")]
    Blocked,
    /// Delivery cancelled.
    #[serde(rename = "cancelled")]
    Cancelled,
}
impl DeliveryState {
    /// Decode the stored identifier; invalid state is not silently defaulted.
    pub fn from_stored(value: &str) -> Option<Self> {
        match value {
            "ready" => Some(Self::Ready),
            "preparing" => Some(Self::Preparing),
            "submitting" => Some(Self::Submitting),
            "retry" => Some(Self::Retry),
            "accepted" => Some(Self::Accepted),
            "failed" => Some(Self::Failed),
            "unknown" => Some(Self::Unknown),
            "blocked" => Some(Self::Blocked),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
    /// Stable value used by storage, presentation and tabular exports.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Preparing => "preparing",
            Self::Submitting => "submitting",
            Self::Retry => "retry",
            Self::Accepted => "accepted",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
            Self::Blocked => "blocked",
            Self::Cancelled => "cancelled",
        }
    }
}

/// A protocol attempt has its own lifecycle; abandoned preparation never implies submission.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttemptState {
    /// Preparing without permission to submit.
    Preparing,
    /// Armed SMTP submission.
    Submitting,
    /// A retryable failure ended this attempt.
    Retry,
    /// SMTP acceptance recorded.
    Accepted,
    /// Permanent failure or exhausted retry budget.
    Failed,
    /// Acceptance may have occurred.
    Unknown,
    /// Configuration or policy needs operator attention.
    Blocked,
    /// Cancelled before further delivery work.
    Cancelled,
    /// Preparation lease expired without arming submission.
    Abandoned,
}
impl AttemptState {
    /// Stable protocol-history label for presentation and storage assertions.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Preparing => "preparing",
            Self::Submitting => "submitting",
            Self::Retry => "retry",
            Self::Accepted => "accepted",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
            Self::Blocked => "blocked",
            Self::Cancelled => "cancelled",
            Self::Abandoned => "abandoned",
        }
    }
    /// Decode stored history without treating queue-only states as attempts.
    pub fn from_stored(value: &str) -> Option<Self> {
        match value {
            "preparing" => Some(Self::Preparing),
            "submitting" => Some(Self::Submitting),
            "retry" => Some(Self::Retry),
            "accepted" => Some(Self::Accepted),
            "failed" => Some(Self::Failed),
            "unknown" => Some(Self::Unknown),
            "blocked" => Some(Self::Blocked),
            "cancelled" => Some(Self::Cancelled),
            "abandoned" => Some(Self::Abandoned),
            _ => None,
        }
    }
}

/// The discriminator selects the lifecycle belonging to a directory entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "status", rename_all = "snake_case")]
pub enum DirectoryState {
    /// Saved draft state, separate from execution.
    Draft(crate::campaign_draft::Status),
    /// Launched campaign lifecycle.
    Campaign(CampaignStatus),
}
