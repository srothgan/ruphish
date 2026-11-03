---
title: Results and exports
description: Analysis filters, event shapes, observation counts, and CSV or XLSX downloads.
sidebar:
  order: 23
---

These endpoints require `view_objects`. Results distinguish observed event totals, unique recipients, and cumulative progression. Export applies the same selection filters to a downloadable snapshot.

## GET `/api/campaigns/{id}/results`

Read filtered counts, recipient rows, events, and time buckets.

**Request:** Query: reporting::Filter. **Response:** `200 OK`, `campaign_reads::Results` as `application/json`.

This independent example shows an active campaign with one public click. The source filter excludes SMTP and campaign-lifecycle events from the observed counts; delivery state remains visible on the recipient row.

```sh
curl --request GET "$RUPHISH_URL/api/campaigns/1/results?source=public&offset=0&limit=50" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

Example response:

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

### Query fields

| Attribute | Type | Contract |
| --- | --- | --- |
| `recipient` | `integer or null` | Recipient row identity. |
| `recipients` | `string` | Comma-separated recipient identities for multi-selection; empty means all. |
| `from` | `integer or null` | Inclusive UTC seconds. |
| `until` | `integer or null` | Exclusive UTC seconds. |
| `event` | `string` | Select recipients with any of these comma-separated event identifiers in the time/source scope, retaining all their scoped events. Empty means no selection. Validated by HTTP. |
| `search` | `string` | Recipient search. |
| `source` | `string` | Observation source supplied by the client; the HTTP boundary validates it. |
| `event_sort` | `EventSort` | Event ordering before pagination. |
| `event_order` | `SortDirection` | Event direction, independent of recipient ordering. |
| `sort` | `RecipientSort` | Recipient ordering before pagination, independent of event ordering. |
| `order` | `SortDirection` | Sort direction; equal values always use ascending recipient identity. |
| `offset` | `integer` | Row offset. |
| `limit` | `integer` | Page length. |
| `format` | `string` | Download format. |
| `scope` | `string` | Download scope. |

| Parameter | Accepted values and behavior |
| --- | --- |
| recipient / recipients | A numeric recipient ID, or comma-separated numeric IDs. Omission or an empty multi-selection selects all. |
| from / until | Inclusive start and exclusive end in Unix seconds; start must precede end. |
| event | Comma-separated keys: `campaign_created`, `email_sent`, `email_error`, `email_opened`, `link_clicked`, `form_submitted`, `email_reported`, `delivery_reconciled`. Selects recipients with any matching event and retains their other scoped events. |
| source | `operator`, `legacy`, `application`, `public`, or `mailbox`; empty means all. |
| sort / order | Recipient sort: `email` (default), `name`, `delivery`, `accepted`, `opened`, `clicked`, `submitted`, `reported`. Direction: `asc` (default) or `desc`. |
| event_sort / event_order | Event sort: `time` (default), `kind`, `source`, or `password_present`. Direction defaults to `desc`. |
| offset / limit | Defaults 0 and 50. Limit must be 1–200. Recipient and event orderings are independent. |
| format / scope | Export only: `csv` or `xlsx`; `events`, `recipients`, or `all`. CSV cannot use `all`. Defaults: `csv`, `events`. |

### Response fields

| Attribute | Type | Contract |
| --- | --- | --- |
| `track_opens` | `boolean` | Whether open counts represent an enabled observation channel. |
| `first_observed` | `ObservationTimes` | First observed UTC second per metric, before pagination; absent without valid evidence. |
| `last_observed` | `ObservationTimes` | Latest observed UTC second per metric, before pagination. |
| `id` | `integer` | Campaign identity. |
| `name` | `string` | Campaign display name. |
| `status` | `CampaignStatus` | Lifecycle state. |
| `as_of` | `integer` | Snapshot UTC seconds. |
| `audience` | `integer` | Unfiltered audience size. |
| `unique` | `Counts` | Unique recipients per observed event type. |
| `progression` | `Counts` | Cumulative recipient progression. |
| `totals` | `Counts` | Effective event counts. |
| `recipient_total` | `integer` | Matching recipients before pagination. |
| `event_total` | `integer` | Matching events before pagination. |
| `recipients` | `ResultRecipient[]` | Recipient page. |
| `events` | `ResultEvent[]` | Event page. |
| `buckets` | `Bucket[]` | Chart intervals. |
| `participant_buckets` | `Bucket[]` | Cumulative distinct participants per action, independent of pagination. |
| `bucket_seconds` | `integer` | Interval width in seconds. |

`Counts` always contains integer `accepted`, `opened`, `clicked`, `submitted`, and `reported` members. `totals` counts observations, including repeats. `unique` counts recipients with each observed event. `progression` also counts submissions as clicks and opens, and clicks as opens; it never infers SMTP acceptance. `first_observed` and `last_observed` use those same five keys with nullable Unix timestamps, without progression inference.

### Recipient, event, and bucket fields

| Attribute | Type | Contract |
| --- | --- | --- |
| `id` | `integer` | Recipient identity. |
| `email` | `string` | Administrator-supplied address. |
| `name` | `string` | Administrator-supplied name. |
| `delivery` | `DeliveryState or null` | Transport outcome, or null when no native delivery record exists. |
| `counts` | `Counts` | Observed event counts. |
| `historical_reported` | `boolean` | Legacy report state without an observed timestamp. |

| Attribute | Type | Contract |
| --- | --- | --- |
| `client` | `Info or null` | Immutable coarse request categories; absent for historical or non-client observations. |
| `edited` | `boolean` | Whether an operator has changed the event values since creation. |
| `id` | `integer` | Event identity. |
| `recipient_id` | `integer or null` | Recipient when resolvable. |
| `email` | `string` | Administrator-supplied address. |
| `time` | `string` | Normalized UTC timestamp. |
| `kind` | `events::RecordedKind` | Event kind. |
| `source` | `Source` | Observation provenance. |
| `password_present` | `boolean or null` | Presence only, when recorded. |
| `username_present` | `boolean or null` | Username presence when explicitly recorded, never inferred from addressing. |

| Attribute | Type | Contract |
| --- | --- | --- |
| `time` | `integer` | UTC bucket start. |
| `counts` | `Counts` | Observed event counts. |

A recorded event kind is tagged JSON, for example:

```json
{"type":"known","value":"link_clicked"}
```

Imported unrecognized kinds use `{"type":"historical","value":"Original label"}`. Presence fields record booleans, not entered usernames or passwords. Null means unknown evidence; it is different from false.

### Client observations

`client` is null when no client observation was collected. Otherwise it has this shape:

```json
{"browser":"firefox","os":"linux","device":"desktop"}
```

| Attribute | Values |
| --- | --- |
| browser | `unknown`, `chrome`, `edge`, `firefox`, `safari`, `opera`, `samsung_internet`, `internet_explorer` |
| os | `unknown`, `windows`, `macos`, `linux`, `android`, `ios`, `chromeos` |
| device | `unknown`, `desktop`, `mobile`, `tablet` |

These are coarse request categories, not verified device identities. Responses exclude raw User-Agent headers, versions, models, and submitted field values.

## GET `/api/campaigns/{id}/export`

Download the filtered snapshot. **Request:** query `reporting::Filter`, no body. **Response:** `200 OK`, CSV bytes or an XLSX workbook, with `Content-Disposition: attachment`.

```sh
curl --request GET "$RUPHISH_URL/api/campaigns/1/export?format=csv&scope=events" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --output campaign-events.csv
```

Example `text/csv; charset=utf-8` response for a selection with no events:

```csv
Event ID,Recipient ID,Email,Time (UTC),Event,Source,Password present,Edited,Browser,Operating system,Device
```

For a workbook containing both tables, request `?format=xlsx&scope=all` and save the response as `.xlsx`. Its media type is `application/vnd.openxmlformats-officedocument.spreadsheetml.sheet`; the response is binary, not JSON. Export contains the selected snapshot rather than just the current on-screen page. CSV cells beginning with spreadsheet formula characters are prefixed to prevent formula execution.
