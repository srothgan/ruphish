//! Destination policy applied to resolved socket addresses, before connecting.
use crate::resources::invalid;
use ipnet::IpNet;
use ruphish_application::resources::Error;
use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};

/// Legacy opt-in internal-host allowlist with link-local protection by default.
#[derive(Clone, Default)]
pub struct Policy {
    allowed: Vec<IpNet>,
}
impl Policy {
    /// Reject invalid configured CIDRs rather than silently dropping policy entries.
    pub fn new(values: &[String]) -> Result<Self, Error> {
        let allowed = values
            .iter()
            .map(|v| {
                v.parse::<IpAddr>()
                    .map(IpNet::from)
                    .or_else(|_| v.parse::<IpNet>())
                    .map_err(|_| invalid("Invalid allowed_internal_hosts address or CIDR"))
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { allowed })
    }
    /// Normalize mapped IPv4 before applying rules so IPv6 cannot bypass a deny.
    pub fn permits(&self, address: IpAddr) -> bool {
        let ip = match address {
            IpAddr::V6(v) => v.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(address),
            _ => address,
        };
        if self.allowed.iter().any(|n| n.contains(&ip)) {
            return true;
        }
        let denied: &[&str] = if self.allowed.is_empty() {
            &["169.254.0.0/16", "fe80::/10"]
        } else {
            &[
                "0.0.0.0/8",
                "127.0.0.0/8",
                "10.0.0.0/8",
                "100.64.0.0/10",
                "172.16.0.0/12",
                "169.254.0.0/16",
                "192.88.99.0/24",
                "192.168.0.0/16",
                "198.51.100.0/24",
                "203.0.113.0/24",
                "224.0.0.0/4",
                "240.0.0.0/4",
                "::/0",
            ]
        };
        !denied
            .iter()
            .any(|n| n.parse::<IpNet>().is_ok_and(|n| n.contains(&ip)))
    }
    /// Resolve once with a deadline and return only addresses authorized for connection.
    pub async fn resolve(&self, host: &str, port: u16) -> Result<Vec<SocketAddr>, Error> {
        let addresses = tokio::time::timeout(
            Duration::from_secs(10),
            tokio::net::lookup_host((host, port)),
        )
        .await
        .map_err(|_| invalid("DNS lookup timed out"))?
        .map_err(|_| invalid("DNS lookup failed"))?;
        let mut checked = Vec::new();
        for addr in addresses {
            if !self.permits(addr.ip()) {
                return Err(invalid("Upstream connection denied to internal host"));
            }
            if !checked.contains(&addr) {
                checked.push(addr);
            }
        }
        if checked.is_empty() {
            return Err(invalid("Host resolved to no addresses"));
        }
        Ok(checked)
    }
    /// Follow at most five redirects, pinning each checked resolution and disabling proxies.
    pub async fn fetch(&self, address: &str) -> Result<(String, String), Error> {
        tokio::time::timeout(Duration::from_secs(30), async {
            let mut url = url::Url::parse(address).map_err(|_| invalid("Invalid site URL"))?;
            for _ in 0..=5 {
                if !matches!(url.scheme(), "http" | "https")
                    || !url.username().is_empty()
                    || url.password().is_some()
                {
                    return Err(invalid(
                        "Site import requires an HTTP or HTTPS URL without credentials",
                    ));
                }
                let host = url
                    .host_str()
                    .ok_or_else(|| invalid("Site URL has no host"))?
                    .trim_start_matches('[')
                    .trim_end_matches(']');
                let addresses = self
                    .resolve(
                        host,
                        url.port_or_known_default()
                            .ok_or_else(|| invalid("Invalid URL port"))?,
                    )
                    .await?;
                let client = reqwest::Client::builder()
                    .no_proxy()
                    .redirect(reqwest::redirect::Policy::none())
                    .resolve_to_addrs(host, &addresses)
                    .timeout(Duration::from_secs(15))
                    .build()
                    .map_err(|_| Error::Internal)?;
                let mut response = client.get(url.clone()).send().await.map_err(|_| {
                    invalid("Site request failed; check its address and TLS certificate")
                })?;
                if response.status().is_redirection() {
                    let location = response
                        .headers()
                        .get(reqwest::header::LOCATION)
                        .and_then(|v| v.to_str().ok())
                        .ok_or_else(|| invalid("Redirect has no valid location"))?;
                    url = url
                        .join(location)
                        .map_err(|_| invalid("Invalid redirect URL"))?;
                    continue;
                }
                if !response.status().is_success() {
                    return Err(invalid(format!(
                        "Site returned HTTP {}",
                        response.status().as_u16()
                    )));
                }
                if response
                    .content_length()
                    .is_some_and(|n| n > crate::resources::MAX_DOCUMENT as u64)
                {
                    return Err(invalid("Imported site exceeds 8 MiB"));
                }
                let mut bytes = Vec::new();
                while let Some(chunk) = response
                    .chunk()
                    .await
                    .map_err(|_| invalid("Could not read imported site"))?
                {
                    if bytes.len() + chunk.len() > crate::resources::MAX_DOCUMENT {
                        return Err(invalid("Imported site exceeds 8 MiB"));
                    }
                    bytes.extend_from_slice(&chunk);
                }
                let source =
                    String::from_utf8(bytes).map_err(|_| invalid("Imported HTML is not UTF-8"))?;
                return Ok((source, url.to_string()));
            }
            Err(invalid("Site exceeded five redirects"))
        })
        .await
        .map_err(|_| invalid("Site import timed out"))?
    }
}
