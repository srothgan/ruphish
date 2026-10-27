//! Event semantics shared by reporting, ingestion and integrations.

/// Evidence for one uniquely identified campaign recipient within a selected scope.
/// Repeated and out-of-order observations only set flags; they never create events.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Observations {
    /// Recorded SMTP acceptance.
    pub accepted: bool,
    /// Recorded image fetch.
    pub opened: bool,
    /// Recorded landing-page visit.
    pub clicked: bool,
    /// Recorded form submission, with or without password presence.
    pub submitted: bool,
    /// Recorded report, independent of interaction progression.
    pub reported: bool,
    /// Explicit positive password presence on a submission.
    pub credentials: bool,
}

impl Observations {
    /// Merge a recorded event. Unknown kinds cannot affect counts.
    pub fn record(&mut self, kind: &str, password_present: bool) {
        match kind {
            "Email Sent" => self.accepted = true,
            "Email Opened" => self.opened = true,
            "Clicked Link" => self.clicked = true,
            "Submitted Data" => {
                self.submitted = true;
                self.credentials |= password_present;
            }
            "Email Reported" => self.reported = true,
            _ => {}
        }
    }
}

/// Distinct-recipient progression counts, not raw event totals.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Progression {
    /// Accepted, opened (including clicks/submissions), clicked (including
    /// submissions), submitted and reported. Acceptance is never inferred.
    pub counts: [i64; 5],
    /// Submitting alone does not imply positive password presence.
    pub credentials_entered: i64,
}

impl Progression {
    /// Add a group of recipients with identical evidence. Adapters may group
    /// flags in SQL to keep aggregation memory independent of audience size.
    /// This is the single owner of progression inference for all summaries.
    pub fn include(&mut self, evidence: Observations, recipients: i64) {
        let clicked = evidence.clicked || evidence.submitted;
        let flags = [
            evidence.accepted,
            evidence.opened || clicked,
            clicked,
            evidence.submitted,
            evidence.reported,
        ];
        for (count, present) in self.counts.iter_mut().zip(flags) {
            *count += i64::from(present) * recipients;
        }
        self.credentials_entered +=
            i64::from(evidence.submitted && evidence.credentials) * recipients;
    }
}

/// Validate an inclusive start/exclusive end window and bounded query size.
pub fn valid_window(from: Option<i64>, until: Option<i64>, limit: usize) -> bool {
    (1..=200).contains(&limit) && !matches!((from, until), (Some(a), Some(b)) if a >= b)
}

/// Stable recipient columns accepted by analysis and export APIs.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum RecipientSort {
    /// Audience email address.
    #[default]
    Email,
    /// Audience display name.
    Name,
    /// Current delivery state.
    Delivery,
    /// Recorded SMTP acceptances.
    Accepted,
    /// Recorded opens.
    Opened,
    /// Recorded visits.
    Clicked,
    /// Recorded submissions.
    Submitted,
    /// Recorded reports, excluding undated historical flags.
    Reported,
}

/// Explicit ordering direction, shared by browser tables and API queries.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    /// Lowest values first.
    #[default]
    Asc,
    /// Highest values first.
    Desc,
}

/// Event-table ordering, independent of the recipient-table selection.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum EventSort {
    /// Observation instant.
    #[default]
    Time,
    /// Recorded event kind.
    Kind,
    /// Observation provenance.
    Source,
    /// Optional credential-presence flag, never participant values.
    PasswordPresent,
}

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
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    /// Imported recipient with no native transport record.
    #[serde(rename = "historical")]
    Historical,
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
            "historical" => Some(Self::Historical),
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
            Self::Historical => "historical",
        }
    }
}

/// Recorded observation provenance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Source {
    /// Imported event history.
    #[serde(rename = "legacy")]
    Legacy,
    /// Application-generated observation.
    #[serde(rename = "application")]
    Application,
    /// Public tracking endpoint.
    #[serde(rename = "public")]
    Public,
    /// Verified reporting mailbox.
    #[serde(rename = "mailbox")]
    Mailbox,
}
impl Source {
    /// Decode the stored identifier; invalid state is not silently defaulted.
    pub fn from_stored(value: &str) -> Option<Self> {
        match value {
            "legacy" => Some(Self::Legacy),
            "application" => Some(Self::Application),
            "public" => Some(Self::Public),
            "mailbox" => Some(Self::Mailbox),
            _ => None,
        }
    }
    /// Stable value used by storage, presentation and tabular exports.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Legacy => "legacy",
            Self::Application => "application",
            Self::Public => "public",
            Self::Mailbox => "mailbox",
        }
    }
}
