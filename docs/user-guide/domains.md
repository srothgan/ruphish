---
title: Verify a campaign domain
description: Prove domain control and HTTPS routing before using an origin in a campaign.
sidebar:
  order: 10
---

Campaign links use an owned, enabled HTTPS origin. Administrators can require verification in [company settings](../administration/settings.md); it is off on fresh installations. When required, verification checks both your DNS proof and whether requests reach this Ruphish installation over trusted HTTPS. With verification off, add and enable the domain; verification controls and checks are inactive.

## Add and verify

1. Open **Resources → Domains** and create a domain with its public HTTPS origin.
2. Publish the displayed TXT record. Use **Copy record name** and **Copy TXT value** to avoid transcription errors.
3. Point the hostname at the public listener or a correctly configured reverse proxy.
4. Ensure the public certificate covers the hostname and is trusted by the running server.
5. Choose **Verify domain** and inspect the completed check.

The TXT name is `_ruphish-verify.<hostname>` and its value starts with `ruphish-verification=`. Keep the proof published for periodic checks. The application also checks a short-lived routing proof, so merely serving a page containing the public TXT value is insufficient.

## Read the status

Verification and enablement are separate. A verified but disabled domain is not available for a new campaign. **Refresh status** reads the result; **Verify domain** requests work. The page shows queued/running work and the latest ten completed attempts.

By default, when verification is required, successful evidence lasts 24 hours and rechecks run every six hours with jitter. Administrators can change these intervals or disable expiry. The latest completed check is authoritative: a failed check revokes verification immediately. Expired evidence pauses new delivery without consuming SMTP retries. Expiry alone does not stop existing links, but a failed recheck or disabling the domain stops new public interactions.

| Diagnostic | What to check |
| --- | --- |
| `domain_txt_unavailable` | DNS resolution from the server's resolver |
| `domain_txt_mismatch` | Exact TXT name/value and the current installation's challenge |
| `domain_destination_denied` | Allowed destinations in installation configuration |
| `domain_https_unavailable` | Certificate chain/hostname, listener, and network reachability |
| `domain_routing_mismatch` | Whether the hostname routes to this instance rather than a generic proxy page |

## Change or remove a domain

The hostname is immutable. Create a new domain for a different host. One account can claim a hostname, including its different ports; creating a second account's duplicate does not grant access.

Disabling stops future delivery/public handling, but cannot recall mail already submitted. Delete only an unreferenced domain. Imported Gophish domains are not approved automatically; old HTTP/IP/dynamic-host campaigns keep their history but require a replacement campaign with an eligible HTTPS origin for new delivery.

Ask the installation operator to configure [listeners, certificates, and proxy trust](../administration/listeners-and-tls.md). Built-in ACME and DNS-provider automation are not provided.
