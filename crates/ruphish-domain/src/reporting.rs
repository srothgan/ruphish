//! Event semantics shared by reporting, ingestion and integrations.

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
