//! DNS control proof and pinned, bounded HTTPS instance verification.
use hickory_resolver::{
    Resolver, TokioResolver,
    config::{NameServerConfig, ResolverConfig},
    net::runtime::TokioRuntimeProvider,
};
use ruphish_application::{domains::*, resources::Error};
use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};

/// Installation-owned policy; never accepted through the domain resource API.
#[derive(Clone, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Proxy networks authorized to supply overwritten forwarding headers.
    pub trusted_proxies: Vec<ipnet::IpNet>,
    /// Installation-owned upstream DNS endpoints; empty uses system configuration.
    pub resolvers: Vec<SocketAddr>,
    /// Explicit internal destinations permitted for HTTPS probes.
    pub allowed_networks: Vec<ipnet::IpNet>,
    /// Optional installation CA PEM file for internal HTTPS.
    pub trusted_ca_path: Option<std::path::PathBuf>,
}

/// Resolver and trusted HTTPS probe configuration.
pub struct DnsHttps {
    resolver: TokioResolver,
    config: Config,
    ca: Option<reqwest::Certificate>,
}
impl DnsHttps {
    /// Build the resolver and optional private CA trust from installation settings.
    pub fn new(config: Config) -> Result<Self, Error> {
        let mut builder = if config.resolvers.is_empty() {
            Resolver::builder_tokio()
                .map_err(|_| Error::Invalid("domain_resolver_configuration_invalid".into()))?
        } else {
            let servers = config
                .resolvers
                .iter()
                .map(|address| {
                    let mut server = NameServerConfig::udp_and_tcp(address.ip());
                    server.trust_negative_responses = false;
                    for connection in &mut server.connections {
                        connection.port = address.port();
                    }
                    server
                })
                .collect();
            Resolver::builder_with_config(
                ResolverConfig::from_name_servers(servers),
                TokioRuntimeProvider::default(),
            )
        };
        builder.options_mut().timeout = Duration::from_secs(3);
        builder.options_mut().attempts = 2;
        let resolver = builder
            .build()
            .map_err(|_| Error::Invalid("domain_resolver_configuration_invalid".into()))?;
        let ca = config
            .trusted_ca_path
            .as_ref()
            .map(|path| {
                let pem = std::fs::read(path)
                    .map_err(|_| Error::Invalid("domain_ca_unreadable".into()))?;
                reqwest::Certificate::from_pem(&pem)
                    .map_err(|_| Error::Invalid("domain_ca_invalid".into()))
            })
            .transpose()?;
        Ok(Self {
            resolver,
            config,
            ca,
        })
    }
    /// Strict public destination policy, independent of legacy import/SMTP behavior.
    pub fn permits(&self, address: IpAddr) -> bool {
        let ip = match address {
            IpAddr::V6(v) => v.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(address),
            _ => address,
        };
        // Metadata/link-local and unspecified destinations are never probe targets.
        if ["169.254.0.0/16", "fe80::/10", "0.0.0.0/8", "::/128"]
            .iter()
            .any(|n| n.parse::<ipnet::IpNet>().is_ok_and(|n| n.contains(&ip)))
        {
            return false;
        }
        if self.config.allowed_networks.iter().any(|n| n.contains(&ip)) {
            return true;
        }
        // Only allocated global-unicast IPv6 space is a default outbound target.
        // Translation, discard-only and other special-purpose ranges need explicit
        // installation policy instead of inheriting broad public-network access.
        if let IpAddr::V6(value) = ip
            && value.segments()[0] & 0xe000 != 0x2000
        {
            return false;
        }
        ![
            "127.0.0.0/8",
            "10.0.0.0/8",
            "100.64.0.0/10",
            "172.16.0.0/12",
            "192.168.0.0/16",
            "192.0.0.0/24",
            "192.0.2.0/24",
            "192.88.99.0/24",
            "198.18.0.0/15",
            "198.51.100.0/24",
            "203.0.113.0/24",
            "224.0.0.0/4",
            "240.0.0.0/4",
            "::1/128",
            "fc00::/7",
            "ff00::/8",
            "2001:db8::/32",
            "2001::/23",
            "3fff::/20",
            "64:ff9b::/96",
            "2002::/16",
        ]
        .iter()
        .any(|n| n.parse::<ipnet::IpNet>().is_ok_and(|n| n.contains(&ip)))
    }
    async fn check(&self, check: &Check, o: &mut Observation) -> Result<(), &'static str> {
        let now = check.started_at;
        if check.domain.challenge_expires <= now {
            return Err("domain_challenge_expired");
        }
        let origin = origin(&check.domain.origin, false)?;
        let records = self
            .resolver
            .txt_lookup(format!("_ruphish-verify.{}.", origin.host))
            .await
            .map_err(|_| "domain_txt_unavailable")?;
        let expected = format!("ruphish-verification={}", check.domain.challenge);
        // Each RR may contain multiple character strings; never concatenate different RRs.
        o.control = records.answers().iter().any(|record| {
            if let hickory_resolver::proto::rr::RData::TXT(txt) = &record.data {
                txt.txt_data
                    .iter()
                    .flat_map(|part| part.iter().copied())
                    .eq(expected.bytes())
            } else {
                false
            }
        });
        if !o.control {
            return Err("domain_txt_mismatch");
        }
        let addresses = self
            .resolver
            .lookup_ip(format!("{}.", origin.host))
            .await
            .map_err(|_| "domain_dns_unavailable")?;
        let addresses: Vec<_> = addresses.iter().collect();
        if addresses.is_empty() || addresses.len() > 16 {
            return Err("domain_dns_address_limit");
        }
        if addresses.iter().any(|ip| !self.permits(*ip)) {
            return Err("domain_destination_denied");
        }
        o.dns = true;
        for address in addresses {
            let mut builder = reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(4))
                .resolve_to_addrs(&origin.host, &[SocketAddr::new(address, origin.port)]);
            if let Some(ca) = &self.ca {
                builder = builder.add_root_certificate(ca.clone());
            }
            let client = builder
                .build()
                .map_err(|_| "domain_probe_configuration_invalid")?;
            let response = client
                .get(format!(
                    "{}/.well-known/ruphish/{}",
                    origin.value, check.domain.challenge
                ))
                .send()
                .await
                .map_err(|_| "domain_https_unavailable")?;
            if !response.status().is_success() {
                return Err("domain_routing_mismatch");
            }
            if response.content_length().is_some_and(|n| n > 128) {
                return Err("domain_routing_mismatch");
            }
            let mut response = response;
            let mut body = Vec::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|_| "domain_routing_unavailable")?
            {
                if body.len() + chunk.len() > 128 {
                    return Err("domain_routing_mismatch");
                }
                body.extend_from_slice(&chunk);
            }
            if body != check.routing_proof.as_bytes() {
                return Err("domain_routing_mismatch");
            }
        }
        o.tls = true;
        o.routing = true;
        Ok(())
    }
}
#[async_trait::async_trait]
impl Verifier for DnsHttps {
    async fn verify(&self, check: &Check) -> Observation {
        let mut result = Observation::default();
        result.code =
            match tokio::time::timeout(Duration::from_secs(45), self.check(check, &mut result))
                .await
            {
                Ok(Ok(())) => "verified",
                Ok(Err(code)) => code,
                Err(_) => "domain_check_timeout",
            };
        result
    }
}
