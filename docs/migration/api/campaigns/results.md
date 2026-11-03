---
title: Read campaign results
description: Migrate GET /api/campaigns/{id}/results to the current Ruphish contract.
sidebar:
  order: 205
prev: {"label": "Choose endpoint comparisons", "link": "/docs/migration/api/"}
next: {"label": "Verify and cut over", "link": "/docs/migration/verification-and-rollback/"}
---

Replace the results response parser. The route remains, but it now returns filtered analysis rather than Gophish's results/timeline object.

| | Gophish 0.12.1 | Ruphish |
| --- | --- | --- |
| Operation | `GET /api/campaigns/{id}/results` | `GET /api/campaigns/{id}/results` |
| Success | `200 OK` | `200 OK` |
| Response type | `models.CampaignResults` | `campaign_reads::Results` |

Use a Bearer API key. The [common changes](../common-changes.md) explain shared access, error handling, metadata, and example conventions.

## Required changes

| Attribute / behavior | Gophish 0.12.1 | Ruphish / client change |
| --- | --- | --- |
| `results` | Array of full recipient objects. | Replaced by paged `recipients`; each has numeric `id`, `email`, `name`, `delivery`, `counts`, and `historical_reported`. |
| Recipient identity | `results[].id` is the public string token. | `recipients[].id` and `events[].recipient_id` are numeric database row IDs. Do not cast the old token. |
| `timeline` | Array with `campaign_id`, `email`, `time`, `message`, string `details`. | Replaced by paged `events` with numeric IDs, RFC 3339 `time`, tagged `kind`, `source`, `edited`, nullable presence flags, and nullable `client`. |
| Event kind | Human-readable `message`. | `{"type":"known","value":"link_clicked"}` or a historical tagged value. Do not compare translated labels. |
| Captured data | `details` could encode submitted values/raw browser information; recipients contained location. | No captured values or IP/geolocation. Presence flags are boolean or null; client information uses coarse categories. |
| Counts | No equivalent aggregate fields in this response. | `audience`, `unique`, `progression`, `totals`, and first/last observation timestamps are explicit. Count keys: accepted/opened/clicked/submitted/reported. |
| Paging and totals | Entire result/timeline lists. | `offset`/`limit` for recipients and `event_offset`/`event_limit` for events; `recipient_total`/`event_total` describe selected totals. |
| Filters | No equivalent selection query. | Recipient/event/source/time/client filters. Keep them fixed across page requests; exports can retrieve the full selected snapshot. |
| Time representation | RFC 3339 strings in results/timeline. | Event time remains a string; `as_of`, first/last observations, and bucket times are Unix seconds. |
| Time series | Absent. | `buckets`, `participant_buckets`, and `bucket_seconds`; see the current reference for their different count semantics. |

## Gophish request

`GET /api/campaigns/{id}/results`

No request body.

## Gophish response

```json
{
  "id": 1,
  "name": "Training campaign",
  "status": "In progress",
  "results": [
    {
      "id": "AbCd1234",
      "email": "ada@example.com",
      "first_name": "Ada",
      "last_name": "Lovelace",
      "position": "Engineer",
      "status": "Clicked Link",
      "ip": "192.0.2.10",
      "latitude": 0,
      "longitude": 0,
      "reported": false,
      "send_date": "2026-11-03T12:00:00Z",
      "modified_date": "2026-11-03T12:00:00Z"
    }
  ],
  "timeline": [
    {
      "campaign_id": 1,
      "email": "ada@example.com",
      "time": "2026-11-03T12:00:00Z",
      "message": "Clicked Link",
      "details": "{\"payload\":{},\"browser\":{\"address\":\"192.0.2.10\",\"user-agent\":\"Example browser\"}}"
    }
  ]
}
```

## Ruphish request

Request type: `Query: reporting::Filter`. No request body.

```sh
curl --request GET "$RUPHISH_URL/api/campaigns/1/results?source=public&offset=0&limit=50" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

## Ruphish response

```json
{
  "track_opens": true,
  "first_observed": {
    "accepted": null,
    "opened": null,
    "clicked": 1793707200,
    "submitted": null,
    "reported": null
  },
  "last_observed": {
    "accepted": null,
    "opened": null,
    "clicked": 1793707200,
    "submitted": null,
    "reported": null
  },
  "id": 1,
  "name": "Training campaign",
  "status": "In progress",
  "as_of": 1793707260,
  "audience": 1,
  "unique": {
    "accepted": 0,
    "opened": 0,
    "clicked": 1,
    "submitted": 0,
    "reported": 0
  },
  "progression": {
    "accepted": 0,
    "opened": 1,
    "clicked": 1,
    "submitted": 0,
    "reported": 0
  },
  "totals": {
    "accepted": 0,
    "opened": 0,
    "clicked": 1,
    "submitted": 0,
    "reported": 0
  },
  "recipient_total": 1,
  "event_total": 1,
  "recipients": [
    {
      "id": 1,
      "email": "ada@example.com",
      "name": "Ada Lovelace",
      "delivery": "accepted",
      "counts": {
        "accepted": 0,
        "opened": 0,
        "clicked": 1,
        "submitted": 0,
        "reported": 0
      },
      "historical_reported": false
    }
  ],
  "events": [
    {
      "client": {
        "browser": "firefox",
        "os": "linux",
        "device": "desktop"
      },
      "edited": false,
      "id": 3,
      "recipient_id": 1,
      "email": "ada@example.com",
      "time": "2026-11-03T12:00:00Z",
      "kind": {
        "type": "known",
        "value": "link_clicked"
      },
      "source": "public",
      "password_present": null,
      "username_present": null
    }
  ],
  "buckets": [
    {
      "time": 1793707200,
      "counts": {
        "accepted": 0,
        "opened": 0,
        "clicked": 1,
        "submitted": 0,
        "reported": 0
      }
    }
  ],
  "participant_buckets": [
    {
      "time": 1793707200,
      "counts": {
        "accepted": 0,
        "opened": 0,
        "clicked": 1,
        "submitted": 0,
        "reported": 0
      }
    }
  ],
  "bucket_seconds": 60
}
```

## Behavior and verification

Requires `view_objects`. The Ruphish example filters to `source=public` to illustrate one observed click: opened progression is 1 while observed opens and accepted counts are 0. The Gophish example is an independent legacy response; its API does not provide this source filter. Recipient and event pages are independent, and a page is not the entire campaign.

Exercise more than one recipient/event page and verify that counts come from the aggregate fields rather than the current page length. Check a historical event and nullable presence/client fields. Update any export parser using the current export reference.

Current contract: [Read campaign results](../../../api/analysis-and-exports.md). Comparison source: [Gophish handler at the pinned version](https://github.com/gophish/gophish/blob/95618469799295e2c0fec980805a2dfbb818816b/controllers/api/campaign.go).
