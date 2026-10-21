//! Campaign delivery rules independent of sockets, storage and wall clocks.

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
/// Effective capture policy is owned by the campaign, not its live page reference.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CapturePolicy {
    /// Record submission events without entered field values.
    pub capture_credentials: bool,
    /// Record password-presence booleans only, never the password.
    pub capture_passwords: bool,
    /// Post-submission redirect template, empty for no redirect.
    pub redirect_url: String,
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
    pub state: &'static str,
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
    current: &str,
    completed: bool,
    retries: u32,
    now: i64,
    outcome: &Result<(), DeliveryFailure>,
    jitter: u8,
) -> Option<DeliveryUpdate> {
    if (current == "unknown" && outcome.is_err()) || (current == "preparing" && outcome.is_ok()) {
        return None;
    }
    let mut next = now;
    let mut count = retries;
    let mut code = "";
    let mut stage = String::new();
    let mut smtp_code = None;
    let state = match outcome {
        Ok(()) => "accepted",
        Err(e) => {
            stage = format!("{:?}", e.stage);
            smtp_code = e.smtp_code;
            match e.kind {
                FailureKind::Unknown => {
                    code = "delivery_outcome_unknown";
                    "unknown"
                }
                FailureKind::Permanent => {
                    code = "smtp_permanent_rejection";
                    "failed"
                }
                FailureKind::Blocked => {
                    code = "smtp_profile_blocked";
                    "blocked"
                }
                FailureKind::Retryable if completed => "cancelled",
                FailureKind::Retryable => {
                    if let Some(delay) = retry_delay(retries, jitter) {
                        next = now.saturating_add(delay);
                        count += 1;
                        code = "smtp_temporary_rejection";
                        "retry"
                    } else {
                        code = "delivery_retry_exhausted";
                        "failed"
                    }
                }
            }
        }
    };
    let cooldown = state == "blocked"
        || (state == "retry"
            && outcome.as_ref().err().is_some_and(|e| {
                matches!(e.stage, Stage::Connect | Stage::Tls | Stage::Authentication)
            }));
    let status = match state {
        "accepted" => "Email Sent",
        "retry" => "Retrying",
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
