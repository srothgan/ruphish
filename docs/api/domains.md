---
title: Domains API
description: Register campaign origins and inspect DNS, routing, and TLS verification.
sidebar:
  order: 12
---

Domain records register the public origin used in campaign links. When installation policy requires verification, adding a record alone does not make it eligible for launch. With verification off, an owned, enabled HTTPS origin is eligible without verification evidence.

| Method | Path | Purpose |
| --- | --- | --- |
| GET, POST | `/api/domains/` | List or create owned origins |
| GET | `/api/domains/validated/` | List origins eligible under current policy |
| GET, PUT, DELETE | `/api/domains/{id}` | Read, edit, or delete an origin |
| POST | `/api/domains/{id}/verify` | Queue verification; returns `202` |

## Create and verify

```json
{
  "name": "Local campaign origin",
  "origin": "https://campaign.test:8443",
  "enabled": true
}
```

Use this only with matching DNS, routing, and trust configuration, such as the local fixture. Origins require HTTPS and a DNS hostname; IP literals and wildcard hosts are not accepted. The origin is immutable after creation. Hostname ownership cannot be bypassed by choosing another port.

The returned resource contains `id`, `name`, `origin`, `hostname`, `enabled`, `revision`, and TXT challenge fields `txt_name`, `txt_value`, and `challenge_expires`. Publish the supplied TXT value, route the origin to the participant listener, and request verification. Read the resource again to observe completion.

`control`, `dns`, `routing`, `tls`, `code`, `status`, `eligible`, and `checking` describe separate aspects of verification. Timestamps such as `checked_at`, `verified_until`, and `next_check_at` use UTC Unix seconds when present. Detail includes recent `history` and any `active_check`; a queued check is not a successful verification.

## Edit and delete

PUT accepts the editable name/enabled state, the unchanged origin, and the expected `revision`. Read responses also provide an ETag. DELETE requires `If-Match` containing the revision; missing preconditions return `428`, and stale revisions return `412`. Referenced origins may be ineligible for deletion.

Disabling or failing verification affects campaign delivery and participant-link behavior. See [domain operation](../user-guide/domains.md) for rechecks, expiry, and recovery; see [outbound connections](../administration/outbound-connections.md) when a permitted test network or trust root is needed.

## Effective verification policy

Domain responses add a `verification` object containing `required`, `status`, `expires_at`, and `next_check_at`. When verification is off, `status` is `not_required` and both timestamps are null. Existing observation fields, including legacy `status` and `verified_until`, remain unchanged; a historical `not_verified` status may coexist with `eligible: true`. Consumers should use `eligible` for campaign availability and `verification` for policy-aware presentation. Manual verification requests return `409` with `FEATURE_DISABLED` while disabled. See [settings](settings.md).
