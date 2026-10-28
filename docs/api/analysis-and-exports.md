---
title: Results and exports API
description: Query effective campaign events and download consistent CSV or XLSX reports.
sidebar:
  order: 13
---

`GET /api/campaigns/{id}/results` returns a filtered snapshot. `GET /api/campaigns/{id}/export` downloads a report from the same filter model. Both require access to the campaign. Results, charts and exports use effective evidence: current edits and operator additions are included, excluded events are omitted, and later system observations continue to contribute independently. Original evidence and correction audits are available through recipient event inspection.

## Filters

| Parameter | Meaning |
| --- | --- |
| `recipient` | One numeric recipient-row ID |
| `recipients` | Comma-separated numeric recipient-row IDs; at most 200 |
| `from` | Inclusive UTC Unix seconds |
| `until` | Exclusive UTC Unix seconds |
| `event` | Comma-separated stable event identifiers (union); omit for all events |
| `source` | `legacy`, `application`, `public`, `mailbox`, or `operator`; omit for all sources |
| `search` | Case-insensitive recipient name/email substring |
| `sort` | Recipient column: `email` (default), `name`, `delivery`, `accepted`, `opened`, `clicked`, `submitted`, or `reported` |
| `order` | Recipient direction: `asc` (default) or `desc` |
| `event_sort` | Event column: `time` (default), `kind`, `source`, or `password_present` |
| `event_order` | Event direction: `desc` (default) or `asc` |
| `offset` | Table offset, default 0 |
| `limit` | Requested table rows, 1–200; default 50 |

Recipient filters intersect when combined. Event types are combined as a union; duplicates are ignored. Invalid filters fail rather than silently broadening the result. Supported event filters are `campaign_created`, `email_sent`, `email_error`, `email_opened`, `link_clicked`, `form_submitted`, `email_reported`, and `delivery_reconciled`. Display labels are not filter identifiers; for example, use `event=email_opened,form_submitted`. Unknown event identifiers or sources return `400`.

Sorting applies to the complete filtered collection before pagination. Recipient text uses case-sensitive Unicode order, and counts sort numerically. Event-kind ordering uses the readable event name shown in the table and exports. Equal recipient values use ascending recipient ID; equal event values use event ID in the selected direction. Presence sorts absent, false, then true in ascending order. Undated historical report flags do not contribute to the recorded report count. Unknown sort columns or directions return `400`.

Changing sort or filters should reset `offset` to zero. Separate requests read current data, so new observations can move rows between pages; an offset is not a frozen snapshot. Exports use the same selected ordering across all matching rows and ignore the displayed page offset/limit.

For example, `/api/campaigns/42/results?limit=25&offset=0` reads the first page for campaign 42. That ID is illustrative; use an execution returned by your installation.

## Snapshot fields

`first_observed` and `last_observed` contain the earliest/latest effective UTC Unix second for each metric across the complete filtered selection, before pagination. Both are objects with required `accepted`, `opened`, `clicked`, `submitted` and `reported` fields; a field is `null` when no valid timestamp exists. These are observed times, not inferred progression timestamps.

The response contains campaign `id`, `name`, `status`, `as_of` (UTC Unix seconds), unfiltered `audience`, filtered `recipient_total` and `event_total`, current `recipients` and `events` pages, chart `buckets`, and `bucket_seconds`.

`unique`, `progression`, `totals`, recipient `counts`, and bucket `counts` are objects with five required numeric fields: `accepted`, `opened`, `clicked`, `submitted`, and `reported`. Empty results contain zero values, not omitted fields or positional arrays. `unique` counts distinct recipients with effective matching events; `totals` counts the events themselves. `progression` includes inferred earlier engagement stages. They answer different questions. See [metric definitions](../user-guide/results/metrics.md).

Recipient rows contain `id`, `email`, `name`, `delivery`, `counts`, and `historical_reported`. Delivery is one of `ready`, `preparing`, `submitting`, `retry`, `accepted`, `failed`, `unknown`, `blocked`, `cancelled`, or explicit `null`. `unknown` means ambiguous transport acceptance; `null` means no native transport record exists. The property is required even when null. The UI and tabular exports label that absence as historical; delivery sorting puts it before recorded states ascending and after them descending. Campaign `status` is `Created`, `Queued`, `In progress`, `Emails Sent`, or `Completed`; the first and fourth values can occur in imported campaign history.

Event rows contain `id`, nullable `recipient_id`, `email`, UTC `time`, `kind`, `source`, nullable `password_present`, and `revision`. The nullable fields must still be present. `source` is `legacy`, `application`, `public`, `mailbox`, or `operator`. Missing presence evidence is not the same as `false`. Historical reported state is separate from an observed report event.

A recognized event uses a classified kind such as `{"type":"known","value":"form_submitted"}`. Unrecognized imported names remain explicit history, for example `{"type":"historical","value":"Proxied request"}`. They remain visible in event rows and exports but never contribute to a known action's metrics. CSV and XLSX retain readable event names. There are no aliases for superseded pre-release reporting formats.

## Downloads

Add `format=csv` or `format=xlsx` and `scope=events`, `recipients`, or, for a workbook, `all`. Defaults are CSV and events. Export uses matching rows independently of the displayed table page; it is bounded to 100,000 rows per sheet and a 32 MiB snapshot. Narrow the filter if an export exceeds its limit.

CSV event columns, in order:

```text
Event ID,Recipient ID,Email,Time (UTC),Event,Source,Password present,Correction revision
```

CSV recipient columns, in order:

```text
Recipient ID,Email,Name,Delivery,Accepted events,Open events,Click events,Submission events,Report events,Historical reported,First Name,Last Name,Position,Send Date (UTC),Modified Date (UTC)
```

XLSX includes a Summary sheet and the selected Events and/or Recipients sheets. Summary records the campaign, snapshot time, filters, and counts. CSV text that could be interpreted as a spreadsheet formula is prefixed to keep it text; XLSX text cells are written explicitly as strings. Do not strip that protection before opening untrusted content in a spreadsheet.

The added audience fields come from the campaign's recipient records, preserving separate names and position alongside the existing display name. Send Date and Modified Date use the stored result timestamps, normalized to UTC with fractional precision retained; null, empty, zero-sentinel and invalid historical dates export as empty cells. Send Date retains legacy semantics: it may be a scheduled or retry time, or an accepted send time, so its presence alone does not establish delivery. Event filters affect observed counts, not these stored timestamps. No date is inferred from other events, and exports include all matching recipients regardless of the table's current page.

Downloads use `Content-Disposition: attachment`, a `campaign-{id}` filename, and `Cache-Control: no-store`. They contain operator-provided recipient identities and selected event metadata, not submitted participant values or IP-derived location. See [export interpretation](../user-guide/results/exports.md).

## Event corrections

`POST /api/campaigns/{campaign}/recipients/{recipient}/events` commits one owner-scoped change. It requires `modify_objects`; browser sessions also require the existing CSRF token and origin checks. Completed campaigns accept corrections without changing their lifecycle or delivery work. The response is the committed event record, with immutable `original`, effective `current`, `source`, `excluded`, and `revision`.

```json
{
  "request_id": "operator-request-2026-0001",
  "event_id": 901,
  "revision": 0,
  "action": "edit",
  "value": {
    "time": "2026-06-02T12:00:00+02:00",
    "kind": "form_submitted",
    "presence": { "username_present": null, "password_present": true }
  }
}
```

The IDs and time above are illustrative. Use the reviewed recipient event and an occurrence within that campaign's known launch/completion boundaries. Times require an explicit offset and are normalized to UTC. Independent events need not be chronologically ordered. Presence accepts only booleans or null, and applies only to `form_submitted`; no participant-entered values or derivatives are accepted. Editable kinds are `email_opened`, `link_clicked`, `form_submitted`, and `email_reported`. SMTP and lifecycle events are read-only here.

| Action | Required values | Effect |
| --- | --- | --- |
| `add` | `event_id: null`, `revision: 0`, and `value` | Create an operator-originated entry |
| `edit` | Existing `event_id`, reviewed `revision`, and `value` | Change that event's time, type or presence indicators; retain its origin |
| `exclude` | Existing `event_id`, reviewed `revision`, `value: null` | Omit that event from effective results |
| `restore` | Existing `event_id`, reviewed `revision`, `value: null` | Restore original values and include the event again |

Use a unique 16 to 64 character alphanumeric/hyphen `request_id`. Retrying the identical command with the same authenticated actor and request identity returns its original acknowledgement. Reusing that identity for another command or writing a stale revision returns `412 Precondition Failed`. After an uncertain response, retry the exact command before making another change. Each mutation and its audit row commit atomically. Administrative additions and corrections do not enqueue participant-event webhooks.

`GET /api/campaigns/{campaign}/recipients/{recipient}/events/{event}` returns the current `record`, up to 50 newest `audit` revisions, and `next_before`. Pass `?before={next_before}` for older revisions. Each audit records the actual operator, server action time, action, and before/after states. An impersonated operation records the real administrator as actor. Occurrence time and correction time are separate. The normal recipient history includes excluded events and their correction state; result exports contain effective events with their source and correction revision.
