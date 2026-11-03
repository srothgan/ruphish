---
title: Connect to internal services
description: Allow Ruphish to reach selected internal mail servers, webhook receivers, and campaign domains.
sidebar:
  order: 29
---

Ruphish checks the destination before contacting mail servers, webhook receivers, imported websites, and campaign-verification endpoints. Public services normally need no private-network exception. An internal service may be blocked until the host explicitly allows its address.

An allowlist entry permits a network destination; it does not replace a provider's username, password, or TLS certificate checks.

## Allow an internal mail server or integration

1. Find the IP address or narrow CIDR range used by the intended service as seen from the Ruphish host or container.
2. Add it to `admin_server.allowed_internal_hosts` in the configuration.
3. Restart Ruphish, then repeat the connection test for the sending profile, reporting mailbox, webhook, or import.

For example, merge this field into your existing `admin_server` object, replacing the address:

```json
{
  "allowed_internal_hosts": ["10.20.0.15/32"]
}
```

With Docker Compose, the equivalent entry under the `ruphish` service is:

```yaml
environment:
  ADMIN_SERVER_ALLOWED_INTERNAL_HOSTS: '["10.20.0.15/32"]'
```

Run `docker compose up -d --wait` to apply it. Keep the hostname supplied by your mail or integration provider in the resource's connection settings so certificate validation can match that name.

The allowlist accepts exact IP addresses and CIDRs, not hostnames. IPv6 integration destinations also need an explicit allowance under the current destination policy. Site imports recheck each resolved address and redirect; allowing one service does not allow unrelated redirect targets.

## Verify a privately hosted campaign domain

Domain verification uses a separate policy. If your campaign hostname resolves to a private server, merge an appropriate `domain_verification` object into the configuration:

```json
{
  "domain_verification": {
    "allowed_networks": ["10.20.0.0/24"],
    "trusted_ca_path": "/etc/ruphish/internal-ca.pem"
  }
}
```

Replace the range and CA path. Omit `trusted_ca_path` when the certificate already chains to a trusted public authority. In Docker, mount the CA file at the path used inside the container. Restart Ruphish and run **Verify domain** again.

The certificate must still match the hostname. Metadata/link-local and unspecified addresses remain prohibited verification targets. If internal DNS is also required, configure `domain_verification.resolvers` with resolver IP addresses and ports.

Use `trusted_proxies` only for an incoming reverse proxy; it does not authorize outbound connections. See [Network access and HTTPS](network-access-and-https.md) and the [configuration reference](configuration-reference.md) for those separate settings.
