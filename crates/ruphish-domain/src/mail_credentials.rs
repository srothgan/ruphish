//! Bind retained mail credentials to their destination and authentication settings.
use crate::validation::{Code, Report};

/// Connection identity at which a saved password may be reused.
pub struct Connection<'a> {
    /// Host passed to the transport, without a scheme, brackets or port.
    pub host: &'a str,
    /// Destination port.
    pub port: u16,
    /// Authentication identity; compared exactly.
    pub username: &'a str,
    /// Implicit TLS rather than STARTTLS negotiation.
    pub implicit_tls: bool,
    /// Whether server certificate verification is disabled.
    pub ignore_cert_errors: bool,
}

impl Connection<'_> {
    /// Retain a password only within its original connection boundary.
    pub fn retain_password(&self, previous: &Self, password: &str) -> Result<String, Report> {
        let same_host = match (
            self.host.parse::<std::net::IpAddr>(),
            previous.host.parse::<std::net::IpAddr>(),
        ) {
            (Ok(current), Ok(previous)) => current == previous,
            _ => self.host.eq_ignore_ascii_case(previous.host),
        };
        if !password.is_empty()
            && (!same_host
                || self.port != previous.port
                || self.username != previous.username
                || self.implicit_tls != previous.implicit_tls
                || (self.ignore_cert_errors && !previous.ignore_cert_errors))
        {
            let mut report = Report::default();
            report.reject(
                "password",
                Code::CredentialReentryRequired,
                "Connection settings changed. Re-enter the password.",
            );
            return Err(report);
        }
        Ok(password.to_owned())
    }
}

/// Parse the same SMTP endpoint for credential binding and the actual connection.
pub fn smtp_address(value: &str) -> Result<(String, u16), &'static str> {
    let url =
        url::Url::parse(&format!("smtp://{value}")).map_err(|_| "Invalid SMTP host or port")?;
    if !url.username().is_empty()
        || url.password().is_some()
        || !matches!(url.path(), "" | "/")
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Invalid SMTP host or port");
    }
    let host = url
        .host_str()
        .ok_or("No SMTP Host specified")?
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_owned();
    let port = url.port().unwrap_or(25);
    if port == 0 {
        return Err("Invalid SMTP port");
    }
    Ok((host, port))
}
