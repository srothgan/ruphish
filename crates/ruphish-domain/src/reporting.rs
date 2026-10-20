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

/// Only recognized observations can be selected by reporting clients.
pub const EVENT_TYPES: &[&str] = &[
    "Campaign Created",
    "Email Sent",
    "Error Sending Email",
    "Email Opened",
    "Clicked Link",
    "Submitted Data",
    "Email Reported",
    "Delivery reconciled",
];

/// Validate an inclusive start/exclusive end window and bounded query size.
pub fn valid_window(from: Option<i64>, until: Option<i64>, limit: usize) -> bool {
    (1..=200).contains(&limit) && !matches!((from, until), (Some(a), Some(b)) if a >= b)
}

/// Retry only outcomes for which another HTTP attempt can plausibly succeed.
pub fn retryable(status: Option<u16>) -> bool {
    matches!(status, None | Some(408 | 429 | 500 | 502 | 503 | 504))
}

/// Bounded exponential retry delay, with deterministic caller-supplied jitter.
pub fn retry_delay(attempt: u32, jitter: u32) -> i64 {
    let base = 30_i64 * (1_i64 << attempt.min(7));
    base.min(3600) + i64::from(jitter % 30)
}
