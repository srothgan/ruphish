//! Integration configuration and retry decisions, independent of transport and storage.
/// An endpoint edit; deliberately not printable or serializable.
pub struct WebhookEdit {
    /// Human-readable name.
    pub name: String,
    /// Destination URL.
    pub url: String,
    /// None preserves the existing secret.
    pub secret: Option<String>,
    /// Receive newly committed events.
    pub active: bool,
    /// Required optimistic revision for edits.
    pub revision: Option<i64>,
}
/// A mailbox edit; secrets never appear in debug output.
pub struct MailboxEdit {
    /// Resource display name.
    pub name: String,
    /// Enable scheduled polling.
    pub enabled: bool,
    /// IMAP host without scheme or port.
    pub host: String,
    /// IMAP port.
    pub port: u16,
    /// Configured service account.
    pub username: String,
    /// None preserves existing password.
    pub password: Option<String>,
    /// Implicit TLS; false requires STARTTLS.
    pub tls: bool,
    /// Folder to monitor.
    pub folder: String,
    /// Optional From-domain matching filter, not authenticated identity.
    pub restrict_domain: String,
    /// Remove successfully correlated reports when safe UID deletion is supported.
    pub delete_reported: bool,
    /// Poll interval in seconds.
    pub frequency: u32,
    /// Expected resource revision.
    pub revision: Option<i64>,
}

/// Safe configuration rejection with the established external diagnostic code.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// Invalid mailbox endpoint, polling interval or credential bounds.
    #[error("mailbox_invalid_configuration")]
    MailboxConfiguration,
    /// A webhook URL cannot be parsed.
    #[error("webhook_invalid_url")]
    WebhookUrl,
    /// Invalid webhook endpoint or label.
    #[error("webhook_invalid_configuration")]
    WebhookConfiguration,
    /// An explicitly supplied signing secret is empty or too large.
    #[error("webhook_secret_required")]
    WebhookSecret,
}
impl WebhookEdit {
    /// Validate configuration without network access.
    pub fn validate(&self) -> Result<(), Error> {
        validate_webhook_edit(self)
    }
}
impl MailboxEdit {
    /// Validate configuration without network access.
    pub fn validate(&self) -> Result<(), Error> {
        validate_mailbox_edit(self)
    }
    /// Only a change of mailbox identity invalidates saved UID progress.
    pub fn resets_checkpoint(&self, host: &str, port: i64, username: &str, folder: &str) -> bool {
        self.host != host
            || i64::from(self.port) != port
            || self.username != username
            || self.folder != folder
    }
}
/// Durable state and delay chosen after one webhook attempt.
#[derive(Debug, PartialEq, Eq)]
pub struct WebhookDecision {
    /// Accepted, retry or failed, preserving persisted state names.
    pub state: &'static str,
    /// Delay recorded for this outcome, including terminal attempts for compatibility.
    pub delay: i64,
}
/// Decide the entire webhook retry policy from sanitized transport evidence.
pub fn webhook_decision(
    code: &str,
    status: Option<u16>,
    attempt: i64,
    jitter: u32,
) -> WebhookDecision {
    let retry = code != "accepted"
        && !matches!(
            code,
            "webhook_invalid_url" | "webhook_signing_failed" | "configuration_changed"
        )
        && retryable(status)
        && attempt < 8;
    WebhookDecision {
        state: if code == "accepted" {
            "accepted"
        } else if retry {
            "retry"
        } else {
            "failed"
        },
        delay: retry_delay(attempt as u32, jitter),
    }
}
/// Successful polls use configured frequency; failures back off up to one hour.
pub fn mailbox_delay(success: bool, frequency: u32, failures: u32) -> i64 {
    if success {
        i64::from(frequency)
    } else {
        (60_i64 << failures.min(6)).min(3600)
    }
}
fn label(s: &str) -> bool {
    !s.trim().is_empty() && s.len() <= 255 && !s.chars().any(char::is_control)
}
fn validate_mailbox_edit(e: &MailboxEdit) -> Result<(), Error> {
    if !label(&e.name)
        || !label(&e.host)
        || (e.host.parse::<std::net::IpAddr>().is_err()
            && (e.host.contains(['/', ':', '@', ' ', '?', '#'])
                || url::Host::parse(&e.host).is_err()))
        || e.port == 0
        || !label(&e.username)
        || !label(&e.folder)
        || !(30..=31540000).contains(&e.frequency)
        || e.password
            .as_ref()
            .is_some_and(|s| s.is_empty() || s.len() > 4096)
        || e.restrict_domain.len() > 253
        || e.restrict_domain.chars().any(char::is_control)
    {
        return Err(Error::MailboxConfiguration);
    }
    Ok(())
}

fn validate_webhook_edit(e: &WebhookEdit) -> Result<(), Error> {
    let url = url::Url::parse(&e.url).map_err(|_| Error::WebhookUrl)?;
    if !label(&e.name)
        || e.url.len() > 1000
        || !matches!(url.scheme(), "https" | "http")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(Error::WebhookConfiguration);
    }
    if e.secret
        .as_ref()
        .is_some_and(|s| s.len() > 1024 || s.is_empty())
    {
        return Err(Error::WebhookSecret);
    }
    Ok(())
}

/// Retry only outcomes for which another HTTP attempt can plausibly succeed.
fn retryable(status: Option<u16>) -> bool {
    matches!(status, None | Some(408 | 429 | 500 | 502 | 503 | 504))
}

/// Bounded exponential retry delay, with deterministic caller-supplied jitter.
fn retry_delay(attempt: u32, jitter: u32) -> i64 {
    let base = 30_i64 * (1_i64 << attempt.min(7));
    base.min(3600) + i64::from(jitter % 30)
}
