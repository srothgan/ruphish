---
title: Create a campaign
description: Migrate POST /api/campaigns/ to the current Ruphish contract.
sidebar:
  order: 200
prev: {"label": "Choose endpoint comparisons", "link": "/docs/migration/api/"}
next: {"label": "Verify and cut over", "link": "/docs/migration/verification-and-rollback/"}
---

Replace name-based resource selections with reviewed IDs before creating a campaign.

| | Gophish 0.12.1 | Ruphish |
| --- | --- | --- |
| Operation | `POST /api/campaigns/` | `POST /api/campaigns/` |
| Success | `201 Created` | `201 Created` |
| Response type | `models.Campaign` | `campaign_details::Campaign` |

Use a Bearer API key. The [common changes](../common-changes.md) explain shared access, error handling, metadata, and example conventions.

## Required changes

| Attribute / behavior | Gophish 0.12.1 | Ruphish / client change |
| --- | --- | --- |
| `groups`, `template`, `page`, `smtp` | Top-level objects selected by `name`. | Replace with `selection.groups[]`, `selection.template`, `selection.page`, `selection.smtp`; each contains integer `id` and optional string `revision`. |
| Reviewed resource versions | No review precondition. | Prepare the command, then send the returned revisions as strings. A resource change requires a new review. |
| `name`, `url` | Strings. | Retained; URL origin must be registered, enabled, and eligible under current domain policy. |
| `launch_date`, `send_by_date` | RFC 3339 strings. | Optional nullable RFC 3339 strings. Omitted/null launch means now; omitted/null send-by means no deadline. Do not send Go zero time. |
| `launch_local`, `send_by_local` | Absent. | Optional local/timezone objects. Each is mutually exclusive with its absolute timestamp counterpart. |
| `policy` | Capture/redirect behavior largely derived from selected content. | Optional object: boolean `track_opens`, nullable string `redirect_url`, optional integer `learning_page_id`. |
| `learning_revision` | Absent. | Optional string for reviewed learning content. See the current reference when using a learning page. |
| Unknown/duplicate/mis-cased fields | Go JSON decoder accepts unknown fields and case-insensitive matches. | Campaign command rejects unknown, duplicate, and incorrectly cased fields. Build a write object; do not echo a read response. |
| `Idempotency-Key` | No operation identity for retry. | Optional nonempty header, up to 128 bytes. Reuse only for the same command; a different command under the key conflicts. |
| Schedule/completion dates | RFC 3339 strings; unset values can be Go zero time. | Nullable RFC 3339 strings. Use null for no deadline or completion. |
| Attribution and `track_opens` | Absent. | Nullable attribution fields and an explicit boolean open-tracking setting. |
| `template`, `page`, `smtp` | Related resource objects loaded from saved resources. | Authored content/sender identity snapshots with resource metadata. SMTP transport stays live; password is omitted and `has_password` added. |
| `results[].id` | String public recipient token. | Still a string token here. Numeric IDs in analysis/delivery are different identifiers. |
| `results[].ip`, `latitude`, `longitude` | Could contain network/location data. | Retained fields are sanitized: empty IP and zero coordinates. |
| `timeline[].details`, `client` | String-encoded details could contain captured values and raw browser data; no `client`. | Sanitized string details plus nullable coarse `client` categories. Imported `client` is null. |

## Gophish request

`POST /api/campaigns/`

```json
{
  "name": "Training campaign",
  "url": "http://127.0.0.1:8081",
  "groups": [
    {
      "name": "Training recipients"
    }
  ],
  "template": {
    "name": "Training message"
  },
  "page": {
    "name": "Training page"
  },
  "smtp": {
    "name": "Training sender"
  },
  "launch_date": "2030-01-01T00:00:00Z"
}
```

## Gophish response

```json
{
  "id": 1,
  "name": "Training campaign",
  "status": "Queued",
  "created_date": "2026-11-03T12:00:00Z",
  "launch_date": "2030-01-01T00:00:00Z",
  "send_by_date": "0001-01-01T00:00:00Z",
  "completed_date": "0001-01-01T00:00:00Z",
  "url": "http://127.0.0.1:8081",
  "template": {
    "id": 1,
    "name": "Training message",
    "modified_date": "2026-11-03T12:00:00Z",
    "subject": "Training notification",
    "text": "Hello {{.FirstName}}",
    "html": "<p>Hello {{.FirstName}}</p>",
    "envelope_sender": "",
    "attachments": []
  },
  "page": {
    "id": 1,
    "name": "Training page",
    "modified_date": "2026-11-03T12:00:00Z",
    "html": "<html><head></head><body><h1>Training page</h1></body></html>",
    "redirect_url": "",
    "capture_credentials": false,
    "capture_passwords": false
  },
  "smtp": {
    "id": 1,
    "name": "Training sender",
    "modified_date": "2026-11-03T12:00:00Z",
    "interface_type": "SMTP",
    "host": "smtp.example.com:587",
    "from_address": "training@example.com",
    "ignore_cert_errors": false,
    "headers": [],
    "username": "training@example.com",
    "password": "example-password"
  },
  "results": [
    {
      "id": "AbCd1234",
      "email": "ada@example.com",
      "first_name": "Ada",
      "last_name": "Lovelace",
      "position": "Engineer",
      "status": "Scheduled",
      "ip": "",
      "latitude": 0.0,
      "longitude": 0.0,
      "reported": false,
      "send_date": "2030-01-01T00:00:00Z",
      "modified_date": "2026-11-03T12:00:00Z"
    }
  ],
  "groups": [
    {
      "id": 1,
      "name": "Training recipients",
      "modified_date": "2026-11-03T12:00:00Z",
      "targets": [
        {
          "email": "ada@example.com",
          "first_name": "Ada",
          "last_name": "Lovelace",
          "position": "Engineer"
        }
      ]
    }
  ]
}
```

## Ruphish request

The localhost URL in this example requires a registered, enabled `http://127.0.0.1:8081` origin and domain verification to be off. For a hosted campaign, use your eligible origin. The selected IDs and revisions must refer to existing resources in this installation.

Request type: `campaign::Create`. Save this body as `request.json`:

```json
{
  "name": "Training campaign",
  "url": "http://127.0.0.1:8081",
  "selection": {
    "groups": [
      {
        "id": 1,
        "revision": "1"
      }
    ],
    "template": {
      "id": 1,
      "revision": "1"
    },
    "page": {
      "id": 1,
      "revision": "1"
    },
    "smtp": {
      "id": 1,
      "revision": "1"
    }
  },
  "launch_date": "2030-01-01T00:00:00Z",
  "send_by_date": null,
  "policy": {
    "track_opens": false,
    "redirect_url": ""
  },
  "launch_local": null,
  "send_by_local": null
}
```

```sh
curl --request POST "$RUPHISH_URL/api/campaigns/" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header 'Idempotency-Key: training-campaign-2030-01' \
  --header "Content-Type: application/json" \
  --data-binary @request.json
```

## Ruphish response

```json
{
  "created_at": "2026-11-03T12:00:00Z",
  "created_by": {
    "id": 1,
    "username": "operator",
    "deleted": false
  },
  "edited_by": {
    "id": 1,
    "username": "operator",
    "deleted": false
  },
  "edited_at": "2026-11-03T12:00:00Z",
  "track_opens": false,
  "id": 1,
  "name": "Training campaign",
  "status": "Queued",
  "created_date": "2026-11-03T12:00:00Z",
  "launch_date": "2030-01-01T00:00:00Z",
  "send_by_date": null,
  "completed_date": null,
  "url": "http://127.0.0.1:8081",
  "template": {
    "created_at": "2026-11-03T12:00:00Z",
    "created_by": {
      "id": 1,
      "username": "operator",
      "deleted": false
    },
    "edited_by": {
      "id": 1,
      "username": "operator",
      "deleted": false
    },
    "edited_at": "2026-11-03T12:00:00Z",
    "revision": 1,
    "id": 1,
    "name": "Training message",
    "modified_date": "2026-11-03T12:00:00Z",
    "subject": "Training notification",
    "text": "Hello {{.FirstName}}",
    "html": "<p>Hello {{.FirstName}}</p>",
    "envelope_sender": "",
    "attachments": []
  },
  "page": {
    "created_at": "2026-11-03T12:00:00Z",
    "created_by": {
      "id": 1,
      "username": "operator",
      "deleted": false
    },
    "edited_by": {
      "id": 1,
      "username": "operator",
      "deleted": false
    },
    "edited_at": "2026-11-03T12:00:00Z",
    "revision": 1,
    "id": 1,
    "name": "Training page",
    "modified_date": "2026-11-03T12:00:00Z",
    "html": "<h1>Training page</h1>",
    "redirect_url": ""
  },
  "smtp": {
    "created_at": "2026-11-03T12:00:00Z",
    "created_by": {
      "id": 1,
      "username": "operator",
      "deleted": false
    },
    "edited_by": {
      "id": 1,
      "username": "operator",
      "deleted": false
    },
    "edited_at": "2026-11-03T12:00:00Z",
    "revision": 1,
    "id": 1,
    "name": "Training sender",
    "modified_date": "2026-11-03T12:00:00Z",
    "interface_type": "SMTP",
    "host": "smtp.example.com:587",
    "from_address": "training@example.com",
    "ignore_cert_errors": false,
    "headers": [],
    "username": "training@example.com",
    "has_password": true
  },
  "results": [
    {
      "id": "AbCd1234",
      "email": "ada@example.com",
      "first_name": "Ada",
      "last_name": "Lovelace",
      "position": "Engineer",
      "status": "Scheduled",
      "ip": "",
      "latitude": 0.0,
      "longitude": 0.0,
      "reported": false,
      "send_date": "2030-01-01T00:00:00Z",
      "modified_date": "2026-11-03T12:00:00Z"
    }
  ],
  "groups": [
    {
      "created_at": "2026-11-03T12:00:00Z",
      "created_by": {
        "id": 1,
        "username": "operator",
        "deleted": false
      },
      "edited_by": {
        "id": 1,
        "username": "operator",
        "deleted": false
      },
      "edited_at": "2026-11-03T12:00:00Z",
      "revision": 1,
      "id": 1,
      "name": "Training recipients",
      "modified_date": "2026-11-03T12:00:00Z",
      "targets": [
        {
          "email": "ada@example.com",
          "first_name": "Ada",
          "last_name": "Lovelace",
          "position": "Engineer"
        }
      ]
    }
  ]
}
```

## Behavior and verification

Call `POST /api/campaigns/prepare` first to resolve the audience, validate content/domain policy, and obtain reviewed versions. Both products deduplicate exact email strings across selected groups and keep the first occurrence; Ruphish exposes duplicate/conflict information during preparation. Launch saves the audience and authored content. Success creates scheduled work, not a promise that mail reached an inbox.

Prepare and create one controlled campaign. Confirm the audience is deduplicated, changed selections are rejected, and a repeated request with the same idempotency key does not create another campaign. See the current reference for preparation and the complete response field tables.

Current contract: [Create a campaign](../../../api/campaigns.md). Comparison source: [Gophish handler at the pinned version](https://github.com/gophish/gophish/blob/95618469799295e2c0fec980805a2dfbb818816b/controllers/api/campaign.go).
