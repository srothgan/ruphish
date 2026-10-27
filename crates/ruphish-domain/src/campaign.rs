//! Campaign delivery rules independent of sockets, storage and wall clocks.
mod state;
pub use state::{AttemptState, CampaignStatus, DeliveryState, DirectoryState};

/// Evidence-based handling of a failed delivery attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureKind {
    /// No acceptance occurred; retry after a persisted delay.
    Retryable,
    /// Explicit rejection or invalid content; no automatic retry.
    Permanent,
    /// Profile configuration or policy needs operator attention.
    Blocked,
    /// Acceptance may have occurred; never automatically resend.
    Unknown,
}
/// Protocol boundary used in sanitized diagnostics and recovery records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Local bounded transport capacity.
    Capacity,
    /// Checked address resolution, connection and greeting.
    Connect,
    /// Transport encryption and certificate validation.
    Tls,
    /// SMTP authentication.
    Authentication,
    /// MAIL/RCPT envelope negotiation.
    Envelope,
    /// DATA command before any message content.
    Data,
    /// Message content and final acceptance response.
    Acceptance,
    /// Rendering or message construction before network submission.
    Content,
}
/// Safe transport failure; raw peer text and credentials are deliberately absent.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{kind:?} delivery failure at {stage:?} (SMTP {smtp_code:?})")]
pub struct DeliveryFailure {
    /// Whether and how this attempt may be retried.
    pub kind: FailureKind,
    /// Last attempted protocol boundary.
    pub stage: Stage,
    /// Explicit SMTP response code, when received.
    pub smtp_code: Option<u16>,
}
impl DeliveryFailure {
    /// Construct a failure without retaining untrusted server diagnostics.
    pub fn new(kind: FailureKind, stage: Stage, smtp_code: Option<u16>) -> Self {
        Self {
            kind,
            stage,
            smtp_code,
        }
    }
}
/// Eight retries after the initial attempt, preserving the supported retry budget.
pub const MAX_RETRIES: u32 = 8;
/// The campaign owns its redirect independently of its live page reference.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InteractionPolicy {
    /// Post-submission redirect template, empty for no redirect.
    pub redirect_url: String,
    /// Selected owned learning page; custom redirect must be empty when present.
    pub learning_page_id: Option<i64>,
}
/// Compute the legacy minute-level distribution over the actual unique audience.
pub fn send_date(launch: i64, send_by: Option<i64>, index: usize, total: usize) -> Option<i64> {
    if total == 0 || index >= total {
        return None;
    }
    let Some(end) = send_by else {
        return Some(launch);
    };
    let span = end.checked_sub(launch)?;
    if span < 0 {
        return None;
    }
    let offset = i128::from(span) * index as i128 / (total as i128 * 60) * 60;
    launch.checked_add(i64::try_from(offset).ok()?)
}
/// Next delay in seconds, with bounded positive jitter and checked exhaustion.
pub fn retry_delay(retries: u32, jitter: u8) -> Option<i64> {
    if retries >= MAX_RETRIES {
        return None;
    }
    let base = 60i64 * (1i64 << (retries + 1));
    Some(base + base * i64::from(jitter) / 2550)
}

/// One decision for all providers; persistence fences generation before applying it.
pub struct DeliveryUpdate {
    /// Durable queue state.
    pub state: DeliveryState,
    /// Earliest next attempt in UTC seconds.
    pub next: i64,
    /// Consumed retry budget.
    pub count: u32,
    /// Safe reason, without peer text.
    pub code: &'static str,
    /// Last protocol stage.
    pub stage: String,
    /// Explicit server reply, if any.
    pub smtp_code: Option<u16>,
    /// Whether this outcome updates the profile circuit breaker.
    pub cooldown: bool,
    /// Legacy recipient status unless a later interaction already exists.
    pub status: &'static str,
}
/// Late failures cannot replace uncertainty; acceptance requires an armed delivery.
pub fn delivery_update(
    current: DeliveryState,
    completed: bool,
    retries: u32,
    now: i64,
    outcome: &Result<(), DeliveryFailure>,
    jitter: u8,
) -> Option<DeliveryUpdate> {
    if (current == DeliveryState::Unknown && outcome.is_err())
        || (current == DeliveryState::Preparing && outcome.is_ok())
    {
        return None;
    }
    let mut next = now;
    let mut count = retries;
    let mut code = "";
    let mut stage = String::new();
    let mut smtp_code = None;
    let state = match outcome {
        Ok(()) => DeliveryState::Accepted,
        Err(e) => {
            stage = format!("{:?}", e.stage);
            smtp_code = e.smtp_code;
            match e.kind {
                FailureKind::Unknown => {
                    code = "delivery_outcome_unknown";
                    DeliveryState::Unknown
                }
                FailureKind::Permanent => {
                    code = "smtp_permanent_rejection";
                    DeliveryState::Failed
                }
                FailureKind::Blocked => {
                    code = "smtp_profile_blocked";
                    DeliveryState::Blocked
                }
                FailureKind::Retryable if completed => DeliveryState::Cancelled,
                FailureKind::Retryable => {
                    if let Some(delay) = retry_delay(retries, jitter) {
                        next = now.saturating_add(delay);
                        count += 1;
                        code = "smtp_temporary_rejection";
                        DeliveryState::Retry
                    } else {
                        code = "delivery_retry_exhausted";
                        DeliveryState::Failed
                    }
                }
            }
        }
    };
    let cooldown = state == DeliveryState::Blocked
        || (state == DeliveryState::Retry
            && outcome.as_ref().err().is_some_and(|e| {
                matches!(e.stage, Stage::Connect | Stage::Tls | Stage::Authentication)
            }));
    let status = match state {
        DeliveryState::Accepted => "Email Sent",
        DeliveryState::Retry => "Retrying",
        _ => "Error",
    };
    Some(DeliveryUpdate {
        state,
        next,
        count,
        code,
        stage,
        smtp_code,
        cooldown,
        status,
    })
}

/// The requested reconciliation cannot apply to the current delivery state.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("This action does not apply to the current delivery state")]
pub struct ResolutionError;

/// Manual recovery is separate from automatic transport outcome processing.
pub fn resolve_delivery(
    state: DeliveryState,
    action: &str,
) -> Result<DeliveryState, ResolutionError> {
    match (state, action) {
        (DeliveryState::Unknown, "confirm_accepted") => Ok(DeliveryState::Accepted),
        (DeliveryState::Unknown, "resend_unknown")
        | (DeliveryState::Failed | DeliveryState::Blocked, "retry") => Ok(DeliveryState::Ready),
        _ => Err(ResolutionError),
    }
}
/// Required configuration projected from legacy names or stable resource identities.
pub struct Fields<'a> {
    /// Campaign label, preserving existing whitespace semantics.
    pub name: &'a str,
    /// Whether at least one group was selected.
    pub groups: bool,
    /// Whether an email template was selected.
    pub template: bool,
    /// Whether a landing page was selected.
    pub page: bool,
    /// Whether a sending profile was selected.
    pub smtp: bool,
    /// Effective launch instant (immediate delivery uses the caller's clock).
    pub launch: i64,
    /// Optional initial distribution end.
    pub send_by: Option<i64>,
}
impl Fields<'_> {
    /// Evaluate independent requirements before expensive resource preparation.
    pub fn validation(&self) -> crate::validation::Report {
        let mut report = self.audience_validation();
        report.extend(self.content_validation());
        report.extend(self.schedule_validation());
        report
    }
    /// Requirements owned by the audience step.
    pub fn audience_validation(&self) -> crate::validation::Report {
        use crate::validation::{Code, Report};
        let mut report = Report::default();
        if self.name.is_empty() {
            report.reject("name", Code::Required, "Campaign name not specified");
        }
        if !self.groups {
            report.reject("selection.groups", Code::Required, "No groups specified");
        }
        report
    }
    /// Requirements owned by the content step.
    pub fn content_validation(&self) -> crate::validation::Report {
        use crate::validation::{Code, Report};
        let mut report = Report::default();
        for (selected, field, message) in [
            (
                self.template,
                "selection.template.id",
                "No email template specified",
            ),
            (self.page, "selection.page.id", "No landing page specified"),
            (
                self.smtp,
                "selection.smtp.id",
                "No sending profile specified",
            ),
        ] {
            if !selected {
                report.reject(field, Code::Required, message);
            }
        }
        report
    }
    /// Distribution end ordering, shared by legacy requests and draft readiness.
    pub fn schedule_validation(&self) -> crate::validation::Report {
        let mut report = crate::validation::Report::default();
        if self.send_by.is_some_and(|end| end < self.launch) {
            report.reject(
                "send_by_local.local",
                crate::validation::Code::ScheduleOrder,
                "The launch date must be before the \"send emails by\" date",
            );
        }
        report
    }
}

/// Validate mutually exclusive post-submission destinations without resolving resources.
pub fn policy_validation(
    learning_page_id: Option<i64>,
    redirect_url: &str,
) -> crate::validation::Report {
    use crate::validation::{Code, Report};
    let mut report = Report::default();
    if learning_page_id.is_some_and(|id| id <= 0) {
        report.reject(
            "policy.learning_page_id",
            Code::Invalid,
            "Choose one valid redirect destination",
        );
    }
    if learning_page_id.is_some() && !redirect_url.is_empty() {
        report.reject(
            "policy.redirect_url",
            Code::RedirectConflict,
            "Choose one valid redirect destination",
        );
    }
    report
}
