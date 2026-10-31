//! Canonical campaign origins and the single domain eligibility rule.

/// An HTTP(S) origin with a fixed DNS name or IP address, never user information or a path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Origin {
    /// Canonical ASCII DNS name or IP address without IPv6 brackets.
    pub host: String,
    /// Explicit or standard scheme port.
    pub port: u16,
    /// HTTP or HTTPS transport.
    pub scheme: &'static str,
    /// Canonical origin without a trailing slash.
    pub value: String,
}

impl Origin {
    /// DNS ownership and trusted HTTPS are both required by the verification policy.
    pub fn verification_supported(&self) -> bool {
        self.scheme == "https" && self.host.parse::<std::net::IpAddr>().is_err()
    }
}

/// Parse an origin or extract the fixed origin of a campaign URL.
/// Expressions are permitted in a campaign path, never in its authority.
pub fn origin(input: &str, campaign: bool) -> Result<Origin, &'static str> {
    if input.trim() != input || input.bytes().any(|c| c.is_ascii_control()) || input.contains('\\')
    {
        return Err("domain_invalid_origin");
    }
    let authority = input
        .split_once("://")
        .ok_or("domain_invalid_origin")?
        .1
        .split(['/', '?', '#'])
        .next()
        .ok_or("domain_invalid_origin")?;
    if authority.is_empty() || authority.contains(['@', '%', '{', '}']) || authority.ends_with(':')
    {
        return Err("domain_invalid_origin");
    }
    let url = url::Url::parse(input).map_err(|_| "domain_invalid_origin")?;
    let scheme = match url.scheme() {
        "https" => "https",
        "http" => "http",
        _ => return Err("domain_invalid_origin"),
    };
    if !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || (!campaign && (url.path() != "/" || url.query().is_some()))
    {
        return Err("domain_invalid_origin");
    }
    let host = match url.host().ok_or("domain_invalid_origin")? {
        url::Host::Ipv4(ip) => {
            let host = ip.to_string();
            if authority.split(':').next() != Some(host.as_str()) {
                return Err("domain_invalid_hostname");
            }
            host
        }
        url::Host::Ipv6(ip) => ip.to_string(),
        url::Host::Domain(host) => {
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
            host
        }
    };
    let port = url.port_or_known_default().ok_or("domain_invalid_origin")?;
    if port == 0 {
        return Err("domain_invalid_origin");
    }
    let authority = if host.contains(':') {
        format!("[{host}]")
    } else {
        host.clone()
    };
    let default_port = if scheme == "https" { 443 } else { 80 };
    let value = if port == default_port {
        format!("{scheme}://{authority}")
    } else {
        format!("{scheme}://{authority}:{port}")
    };
    Ok(Origin {
        host,
        port,
        scheme,
        value,
    })
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

/// Validate an operator label and origin without network access.
pub fn validate_edit(name: &str, value: &str) -> Result<Origin, crate::validation::Report> {
    use crate::validation::{Code, Report};
    let mut report = Report::default();
    let parsed = origin(value, false);
    if let Err(message) = parsed.as_ref() {
        report.reject("origin", Code::Invalid, message);
    }
    if name.trim().is_empty() || name.len() > 200 {
        report.reject("name", Code::Invalid, "domain_invalid_name");
    }
    report.finish()?;
    parsed.map_err(|message| {
        let mut report = Report::default();
        report.reject("origin", Code::Invalid, message);
        report
    })
}
