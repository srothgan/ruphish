---
title: Read a campaign
description: Migrate GET /api/campaigns/{id} to the current Ruphish contract.
sidebar:
  order: 203
prev: {"label": "Choose endpoint comparisons", "link": "/docs/migration/api/"}
next: {"label": "Verify and cut over", "link": "/docs/migration/verification-and-rollback/"}
---

Update the campaign parser for shared visibility, nullable dates, snapshots, and sanitized history.

| | Gophish 0.12.1 | Ruphish |
| --- | --- | --- |
| Operation | `GET /api/campaigns/{id}` | `GET /api/campaigns/{id}` |
| Success | `200 OK` | `200 OK` |
| Response type | `models.Campaign` | `campaign_details::Campaign` |

Use a Bearer API key. The [common changes](../common-contract-changes.md) explain shared access, error handling, metadata, and example conventions.

## Required changes

| Attribute / behavior | Gophish 0.12.1 | Ruphish / client change |
| --- | --- | --- |
| Visibility | Campaigns selected by authenticated owner. | Shared installation campaigns, subject to permissions. |
| `id`, `name`, `status`, `url` | Integer ID and strings. | Retained. `status` is campaign lifecycle, not SMTP outcome. |
| Schedule/completion dates | RFC 3339 strings; unset values can be Go zero time. | Nullable RFC 3339 strings. Use null for no deadline or completion. |
| Attribution and `track_opens` | Absent. | Nullable attribution fields and an explicit boolean open-tracking setting. |
| `template`, `page`, `smtp` | Related resource objects loaded from saved resources. | Authored content/sender identity snapshots with resource metadata. SMTP transport stays live; password is omitted and `has_password` added. |
| `results[].id` | String public recipient token. | Still a string token here. Numeric IDs in analysis/delivery are different identifiers. |
| `results[].ip`, `latitude`, `longitude` | Could contain network/location data. | Retained fields are sanitized: empty IP and zero coordinates. |
| `timeline[].details`, `client` | String-encoded details could contain captured values and raw browser data; no `client`. | Sanitized string details plus nullable coarse `client` categories. Imported `client` is null. |

## Gophish request

`GET /api/campaigns/{id}`

No request body.

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
  "timeline": [
    {
      "campaign_id": 1,
      "email": "",
      "time": "2026-11-03T12:00:00Z",
      "message": "Campaign Created",
      "details": ""
    }
  ]
}
```

## Ruphish request

No request body.

```sh
curl --request GET "$RUPHISH_URL/api/campaigns/1" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
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
  "timeline": [
    {
      "client": null,
      "campaign_id": 1,
      "email": "",
      "time": "2026-11-03T12:00:00Z",
      "message": "Campaign Created",
      "details": ""
    }
  ]
}
```

## Behavior and verification

Requires `view_objects`. These full projections can include substantial content and history. Empty `results` and `timeline` are omitted; source `groups` are supplied in creation acknowledgements, not ordinary reads. New paged directory/overview endpoints are available through the current reference.

Check one imported campaign and one newly launched campaign, including missing historical attribution and an absent timeline. Do not use a public recipient token in a numeric delivery route.

Current contract: [Read a campaign](../../../api/campaigns.md). Comparison source: [Gophish handler at the pinned version](https://github.com/gophish/gophish/blob/95618469799295e2c0fec980805a2dfbb818816b/controllers/api/campaign.go).
