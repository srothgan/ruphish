---
title: Read a campaign summary
description: Migrate GET /api/campaigns/{id}/summary to the current Ruphish contract.
sidebar:
  order: 204
prev: {"label": "Choose endpoint comparisons", "link": "/docs/migration/api/"}
next: {"label": "Verify and cut over", "link": "/docs/migration/verification-and-rollback/"}
---

Keep the summary structure, but update metadata handling and the meaning of sent/engagement counts.

| | Gophish 0.12.1 | Ruphish |
| --- | --- | --- |
| Operation | `GET /api/campaigns/{id}/summary` | `GET /api/campaigns/{id}/summary` |
| Success | `200 OK` | `200 OK` |
| Response type | `models.CampaignSummary` | `campaign_details::Summary` |

Use a Bearer API key. The [common changes](../common-changes.md) explain shared access, error handling, metadata, and example conventions.

## Required changes

| Attribute / behavior | Gophish 0.12.1 | Ruphish / client change |
| --- | --- | --- |
| Summary fields | `id`, `name`, dates, `status`, and `stats`. | Retained, with nullable dates, attribution, and `track_opens`. |
| `stats.total` | Integer audience size. | Same unit: recipients. |
| `stats.sent` | Derived from recipient status; later engagement implied sent. | Recorded SMTP acceptance progression; a click alone does not establish acceptance. |
| `stats.opened`, `stats.clicked` | Cumulative status-derived counts. | Cumulative progression under current rules. Distinguish these from raw observed-event counts. |
| `stats.submitted_data`, `stats.email_reported`, `stats.error` | Integer submission/report/error counts. | Retained names; interpret with current observation and delivery behavior, not an assumed one-to-one old status model. |

## Gophish request

`GET /api/campaigns/{id}/summary`

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
  "stats": {
    "total": 1,
    "sent": 0,
    "opened": 0,
    "clicked": 0,
    "submitted_data": 0,
    "email_reported": 0,
    "error": 0
  }
}
```

## Ruphish request

No request body.

```sh
curl --request GET "$RUPHISH_URL/api/campaigns/1/summary" \
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
  "stats": {
    "total": 1,
    "sent": 0,
    "opened": 0,
    "clicked": 0,
    "submitted_data": 0,
    "email_reported": 0,
    "error": 0
  }
}
```

## Behavior and verification

Requires `view_objects`. Use the results endpoint when you need raw events, distinct observed recipients, time buckets, or filtered analysis. Do not equate an inferred open with an observed pixel request.

Compare a campaign with clicks/submissions but incomplete historical send evidence. Your client must not manufacture SMTP acceptance from those interactions.

Current contract: [Read a campaign summary](../../../api/campaigns.md). Comparison source: [Gophish handler at the pinned version](https://github.com/gophish/gophish/blob/95618469799295e2c0fec980805a2dfbb818816b/controllers/api/campaign.go).
