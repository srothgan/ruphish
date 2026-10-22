---
title: Analysis and exports API
description: Query observed campaign events and download consistent CSV or XLSX reports.
sidebar:
  order: 13
---

`GET /api/campaigns/{id}/analysis` returns a filtered snapshot. `GET /api/campaigns/{id}/export` downloads a report from the same filter model. Both require access to the campaign.

## Filters

| Parameter | Meaning |
| --- | --- |
| `recipient` | One numeric recipient-row ID |
| `recipients` | Comma-separated numeric recipient-row IDs; at most 200 |
| `from` | Inclusive UTC Unix seconds |
| `until` | Exclusive UTC Unix seconds |
| `event` | Exact supported event name; omit for all events |
| `source` | `legacy`, `application`, `public`, or `mailbox`; omit for all sources |
| `search` | Case-insensitive recipient name/email substring |
| `offset` | Table offset, default 0 |
| `limit` | Requested table rows, 1–200; default 50 |

Recipient filters intersect when combined. Invalid filters fail rather than silently broadening the result. Supported event filters are `Campaign Created`, `Email Sent`, `Error Sending Email`, `Email Opened`, `Clicked Link`, `Submitted Data`, `Email Reported`, and `Delivery reconciled`. Do not translate these wire values into UI labels before sending them back to the API.

For example, `/api/campaigns/42/analysis?limit=25&offset=0` reads the first page for campaign 42. That ID is illustrative; use an execution returned by your installation.

## Snapshot fields

The response contains campaign `id`, `name`, `status`, `as_of` (UTC Unix seconds), unfiltered `audience`, filtered `recipient_total` and `event_total`, current `recipients` and `events` pages, chart `buckets`, and `bucket_seconds`.

`unique`, `progression`, `totals`, recipient `counts`, and bucket `counts` use this fixed order: **accepted, opened, clicked, submitted, reported**. `unique` counts distinct recipients with observed matching events; `totals` counts the events themselves. `progression` includes inferred earlier engagement stages. They answer different questions. See [metric definitions](../user-guide/results/metrics.md).

Recipient rows contain `id`, `email`, `name`, `delivery`, `counts`, and `historical_reported`. Event rows contain `id`, optional `recipient_id`, `email`, UTC `time`, `kind`, `source`, and optional `password_present`. Missing presence data is not the same as `false`. Historical reported state is separate from an observed report event.

## Downloads

Add `format=csv` or `format=xlsx` and `scope=events`, `recipients`, or, for a workbook, `all`. Defaults are CSV and events. Export uses matching rows independently of the displayed table page; it is bounded to 100,000 rows per sheet and a 32 MiB snapshot. Narrow the filter if an export exceeds its limit.

CSV event columns, in order:

```text
Event ID,Recipient ID,Email,Time (UTC),Event,Source,Password present
```

CSV recipient columns, in order:

```text
Recipient ID,Email,Name,Delivery,Accepted events,Open events,Click events,Submission events,Report events,Historical reported
```

XLSX includes a Summary sheet and the selected Events and/or Recipients sheets. Summary records the campaign, snapshot time, filters, and counts. CSV text that could be interpreted as a spreadsheet formula is prefixed to keep it text; XLSX text cells are written explicitly as strings. Do not strip that protection before opening untrusted content in a spreadsheet.

Downloads use `Content-Disposition: attachment`, a `campaign-{id}` filename, and `Cache-Control: no-store`. They contain operator-provided recipient identities and selected event metadata, not submitted participant values or IP-derived location. See [export interpretation](../user-guide/results/exports.md).
