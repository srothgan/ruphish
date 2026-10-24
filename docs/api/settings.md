---
title: Settings API
description: Installation policy and confirmed closure of active campaigns.
sidebar:
  order: 18
---

These additive endpoints require an authenticated administrator with `modify_system`. API keys and browser sessions are supported; browser writes require CSRF protection. Impersonated browser sessions cannot use these endpoints.

| Method | Path | Result |
| --- | --- | --- |
| GET | `/api/settings` | `200`, policy resource and revision ETag |
| PUT | `/api/settings` | `200`, atomically saved policy resource |
| POST | `/api/campaigns/complete-all/preview` | `200`, expiring confirmation token and counts |
| POST | `/api/campaigns/complete-all` | `200`, durable closure result |

## Policy resource

GET returns `revision`, `policy`, `updated_at` (UTC Unix seconds), and nullable `updated_by` (administrator ID). PUT requires `If-Match` with the current revision and a complete object shaped as follows:

```json
{
  "policy": {
    "verification_enabled": false,
    "verification_expires": true,
    "verification_validity_seconds": 86400,
    "verification_recheck_seconds": 21600,
    "reporting_mailboxes_enabled": false,
    "webhooks_enabled": false
  }
}
```

This example shows defaults without existing integration configurations. During upgrade, each integration feature starts enabled if configurations of that type already exist, regardless of their individual enabled states; domain verification starts off. `verification_expires` applies only when verification is enabled. Validity must be 3,600 to 31,536,000 seconds; rechecks must be 300 to 604,800 seconds. When expiry is selected, recheck interval plus jitter (the smaller of 10% of the interval and 300 seconds) must not exceed half the validity window. Invalid policy or unknown fields returns `400`; missing/invalid write revision returns `428`; stale revision returns `412`. Rejected writes leave policy and associated work unchanged.

Feature flags are also included in authenticated `/auth/session` responses for UI presentation. They do not grant permissions. See [company settings](../administration/settings.md) for disablement, upgrade defaults, and effects on active work.

## Confirm campaign closure

POST the preview endpoint without a body. It returns `token`, `expires_at` (Unix seconds), `campaigns`, and `pending_deliveries`. The token belongs to the requesting administrator, expires after five minutes, and identifies the exact active campaign set across owners. Drafts are excluded.

Send the token back with the exact confirmation string:

```json
{"token":"replace-with-the-preview-token","confirmation":"CLOSE ALL"}
```

A successful result contains `campaigns`, `cancelled_deliveries`, and `completed_at`. Completion and the saved result commit together. Repeating the same token returns the original result even after its original deadline or process restart. Newly created campaigns are unaffected by a replay. An expired unexecuted token or incorrect confirmation returns `400`; a missing or another administrator's token returns `404`; a changed campaign set returns `409`. Request a fresh preview after those failures.

The action closes scheduled and running campaigns, cancels unsent deliveries, and preserves results. Already submitted or externally in-flight mail cannot be recalled. No persistent pause state is created.
