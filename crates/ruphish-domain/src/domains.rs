//! Canonical campaign origins and the single domain eligibility rule.

/// An HTTPS origin with an exact DNS hostname, never user information or a path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Origin {
    /// Canonical ASCII hostname.
    pub host: String,
    /// Explicit or standard HTTPS port.
    pub port: u16,
    /// Canonical origin without a trailing slash.
    pub value: String,
}

/// Parse an origin or extract the fixed origin of a campaign URL.
/// Expressions are permitted in a campaign path, never in its authority.
pub fn origin(input: &str, campaign: bool) -> Result<Origin, &'static str> {
    if input.trim() != input || input.bytes().any(|c| c.is_ascii_control()) || input.contains('\\')
    {
        return Err("domain_invalid_origin");
    }
    let url = url::Url::parse(input).map_err(|_| "domain_invalid_origin")?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || (!campaign && (url.path() != "/" || url.query().is_some()))
    {
        return Err("domain_invalid_origin");
    }
    let Some(url::Host::Domain(host)) = url.host() else {
        return Err("domain_dns_name_required");
    };
    let host = host.strip_suffix('.').unwrap_or(host).to_ascii_lowercase();
    if host.len() > 253
        || !host.contains('.')
        || host.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        })
    {
        return Err("domain_invalid_hostname");
    }
    let port = url.port_or_known_default().ok_or("domain_invalid_origin")?;
    if port == 0 {
        return Err("domain_invalid_origin");
    }
    let value = if port == 443 {
        format!("https://{host}")
    } else {
        format!("https://{host}:{port}")
    };
    Ok(Origin { host, port, value })
}

/// Operational readiness is derived from evidence, never a separately editable flag.
pub fn eligibility(enabled: bool, verified_until: i64, now: i64) -> Result<(), &'static str> {
    if !enabled {
        Err("domain_disabled")
    } else if verified_until == 0 {
        Err("domain_not_verified")
    } else if verified_until <= now {
        Err("domain_check_expired")
    } else {
        Ok(())
    }
}

/// Evidence lifetime and refresh/retry policy are owned here, not by campaign forms.
pub const EVIDENCE_LIFETIME: i64 = 24 * 3600;
/// Refresh before evidence expires, leaving room for transient DNS failures.
pub const REFRESH_INTERVAL: i64 = 6 * 3600;
/// A newly issued proof must be verified within this period.
pub const CHALLENGE_LIFETIME: i64 = 7 * 24 * 3600;

/// Bounded exponential retry independent of SMTP attempts.
pub fn retry_delay(failures: u32) -> i64 {
    (30_i64 << failures.min(6)).min(1800)
}

/// Domain validity is independent of administrative enablement and active work.
pub fn verification_status(verified_until: i64, checked_at: i64, now: i64) -> &'static str {
    if verified_until > now {
        "verified"
    } else if verified_until > 0 {
        "expired"
    } else if checked_at > 0 {
        "failed"
    } else {
        "not_verified"
    }
}

/// Stable spreading of periodic work, within five minutes of the six-hour cadence.
pub fn refresh_delay(domain_id: i64) -> i64 {
    REFRESH_INTERVAL + domain_id.wrapping_mul(97).rem_euclid(601) - 300
}
