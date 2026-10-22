---
title: Permit outbound connections
description: Allow selected internal services without disabling destination checks.
sidebar:
  order: 5
---

Ruphish checks destinations before contacting imported sites and configured integrations. An empty internal allowlist does not disable filtering.

For site imports and integration connections, add exact IP addresses or CIDRs to `admin_server.allowed_internal_hosts` when a private, loopback, or otherwise restricted service is intentional. The conservative integration policy also requires explicit allowances for IPv6. Hostnames alone are not internal allowlist entries.

Site imports recheck resolved addresses and every redirect. Connections use the checked socket address while retaining the original hostname for certificate validation. An allowance does not disable TLS identity checks or make an unrelated redirected destination trusted.

Domain-verification probes have a separate `domain_verification.allowed_networks` policy and optional `trusted_ca_path`. Metadata/link-local and unspecified addresses remain prohibited probe targets even with the verification configuration. Configure the policy that belongs to the failing operation rather than widening unrelated permissions.

After a change, restart the process and repeat the relevant connection/domain validation. Keep exceptions scoped to the service you intend to reach; the [local fixture](../contributing/testing/local-fixture.md) shows explicit exceptions for its own services.
