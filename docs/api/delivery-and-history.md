---
title: Delivery and history
description: SMTP queue states, attempts, pagination, and explicit delivery recovery.
sidebar:
  order: 22
---

Delivery endpoints expose SMTP work independently of participant interactions. Reads require `view_objects`; recovery requires `modify_objects`. Numeric recipient IDs on these paths are internal result-row IDs, not public `rid` tokens.

## GET `/api/campaigns/{id}/delivery`

Read the queue state, attempt summaries, and recent campaign activity.

**Request:** Query: campaign::request::DeliveryPage. **Response:** `200 OK`, `campaign_reads::Delivery` as `application/json`.

```sh
curl --request GET "$RUPHISH_URL/api/campaigns/1/delivery?sort=email&order=asc&limit=50" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

Example response:

```json
{
  "next_cursor": null,
  "campaign": {
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
    "id": 1,
    "name": "Training campaign",
    "status": "Queued",
    "launch_date": "2030-01-01T00:00:00Z",
    "send_by_date": null,
    "url": "http://127.0.0.1:8081",
    "template": {
      "id": 1,
      "name": "Training message"
    },
    "page": {
      "id": 1,
      "name": "Training page"
    },
    "smtp": {
      "id": 1,
      "name": "Training sender"
    },
    "subject": "Training notification",
    "profile_sender": "training@example.com",
    "template_sender": "",
    "total": 1,
    "policy": {
      "track_opens": false,
      "redirect_url": ""
    },
    "domain_id": 1,
    "domain_block": null
  },
  "counts": {
    "ready": 1
  },
  "items": [
    {
      "id": 1,
      "rid": "AbCd1234",
      "email": "ada@example.com",
      "attempt_count": 0,
      "state": "ready",
      "due": 1893456000,
      "retries": 0,
      "generation": 0,
      "code": "",
      "attempts": []
    }
  ],
  "next_after": null,
  "policy": {
    "track_opens": false,
    "redirect_url": ""
  },
  "mailer_enabled": true,
  "activity": []
}
```

### Query fields

| Attribute | Type | Contract |
| --- | --- | --- |
| `after` | `integer` | Continuation returned by the previous page. |
| `limit` | `integer or null` | Requested page size. |
| `sort` | `pagination::DeliverySort or null` | Optional sort field. |
| `order` | `SortDirection or null` | Optional sort direction. |
| `cursor` | `string` | Opaque sorted continuation token. |

With `sort`, use `priority`, `email`, `state`, `due`, or `attempt_count`; `order` is `asc` or `desc`. Keep `after` at zero and supply returned `next_cursor` as URL-encoded `cursor`. A sorted page size is 1–200, default 50. Keep campaign, caller, sort, and order unchanged between pages. Without sorting, use numeric `after` and returned `next_after`; `order` and `cursor` are not accepted in that mode.

### Response fields

| Attribute | Type | Contract |
| --- | --- | --- |
| `next_cursor` | `string or null` | Selection-bound continuation for explicit sorted queries. |
| `campaign` | `Header` | Campaign metadata. |
| `counts` | `object` | Sparse state counts: omitted states have zero rows; the map itself is required. |
| `items` | `DeliveryRow[]` | Recipients in the requested page. |
| `next_after` | `integer or null` | Next recipient cursor. |
| `policy` | `Policy or null` | Effective presence-only policy. |
| `mailer_enabled` | `boolean` | Whether this process runs delivery workers. |
| `activity` | `Activity[]` | Most recent campaign events. |

| Attribute | Type | Contract |
| --- | --- | --- |
| `id` | `integer` | Internal recipient identity. |
| `rid` | `string` | Opaque public recipient identifier. |
| `email` | `string` | Administrator-supplied recipient address. |
| `attempt_count` | `integer` | Total persisted attempts. |
| `state` | `DeliveryState` | Transport state. |
| `due` | `integer` | Next or last due UTC seconds. |
| `retries` | `integer` | Automatic retries consumed. |
| `generation` | `integer` | Conditional mutation generation. |
| `code` | `string` | Safe diagnostic code. |
| `attempts` | `Attempt[]` | Bounded inline attempt records. |

`campaign` is the [campaign context](campaigns.md#context-fields). `counts` is a state-to-integer map; absent states should be treated as zero. Null continuation fields mean no further page. `policy` has the same shape as campaign context.

### Delivery states

| State | Meaning |
| --- | --- |
| ready | Waiting until its due time. |
| preparing | Preparing content before SMTP submission. |
| submitting | SMTP submission is in progress. |
| retry | Waiting for an automatic retry. |
| accepted | SMTP acceptance was recorded; this is not confirmation of inbox delivery. |
| failed | Permanent failure or retry exhaustion. |
| unknown | SMTP acceptance may have occurred; automatic resend is disabled. |
| blocked | Configuration or policy requires operator action. |
| cancelled | Further delivery work is cancelled. |

### Attempt and activity fields

| Attribute | Type | Contract |
| --- | --- | --- |
| `generation` | `integer` | Attempt identity. |
| `started` | `integer` | UTC start seconds. |
| `finished` | `integer or null` | UTC completion seconds if finished. |
| `state` | `AttemptState` | Recorded outcome. |
| `stage` | `string` | Protocol stage if known. |
| `smtp_code` | `integer or null` | SMTP response if known. |
| `message_id` | `string` | Delivery correlation identifier. |

Attempt states are `preparing`, `submitting`, `retry`, `accepted`, `failed`, `unknown`, `blocked`, `cancelled`, and `abandoned`. An abandoned preparation never reached an authorized SMTP submission. Attempt timestamps are Unix seconds. `smtp_code` is null when no SMTP reply code was received.

| Attribute | Type | Contract |
| --- | --- | --- |
| `client` | `Info or null` | Immutable coarse request categories, when observed. |
| `id` | `integer` | Event identity. |
| `time` | `string or null` | Preserved timestamp. |
| `email` | `string` | Administrator-supplied recipient address. |
| `message` | `string` | Event kind. |
| `details` | `string` | Sanitized event metadata, never participant values. |

## GET `/api/campaigns/{id}/recipients/{recipient_id}/history`

Read one recipient’s attempt history and editable event records.

**Request:** Query: campaign::request::HistoryPage. **Response:** `200 OK`, `campaign_reads::History` as `application/json`.

This independent example shows a recipient whose delivery has completed. Optional `before_attempt` and `before_event` are exclusive cursors; pass the corresponding `next_attempt` and `next_event` values to continue each history.

```sh
curl --request GET "$RUPHISH_URL/api/campaigns/1/recipients/1/history" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

Example response:

```json
{
  "result_id": 1,
  "email": "ada@example.com",
  "attempts": [
    {
      "generation": 1,
      "started": 1793707200,
      "finished": 1793707201,
      "state": "accepted",
      "stage": "",
      "smtp_code": null,
      "message_id": "<example-message@example.com>"
    }
  ],
  "events": [
    {
      "record": {
        "client": null,
        "id": 2,
        "source": "application",
        "revision": 1,
        "edited": false,
        "current": {
          "time": "2026-11-03T12:00:01Z",
          "kind": {
            "type": "known",
            "value": "email_sent"
          },
          "presence": {
            "username_present": null,
            "password_present": null
          }
        }
      },
      "id": 2,
      "time": "2026-11-03T12:00:01Z",
      "at": 1793707201,
      "message": "Email Sent",
      "details": ""
    }
  ],
  "next_attempt": null,
  "next_event": null
}
```

| Attribute | Type | Contract |
| --- | --- | --- |
| `result_id` | `integer` | Recipient identity. |
| `email` | `string` | Administrator-supplied address. |
| `attempts` | `Attempt[]` | Attempt page. |
| `events` | `HistoryEvent[]` | Event page. |
| `next_attempt` | `integer or null` | Older-attempt continuation. |
| `next_event` | `integer or null` | Older-event continuation. |

| Attribute | Type | Contract |
| --- | --- | --- |
| `record` | `event_edit::Record` | Current event values, origin and edit state. |
| `id` | `integer` | Event identity. |
| `time` | `string or null` | Preserved timestamp. |
| `at` | `integer or null` | UTC seconds if parseable. |
| `message` | `string` | Event kind. |
| `details` | `string` | Sanitized metadata. |

Each event’s `record` is the current [editable event record](event-corrections.md#response-fields). Historical event details are sanitized JSON strings.

## POST `/api/campaigns/{id}/delivery/{recipient_id}`

Apply an explicit recovery decision to the reviewed delivery generation.

**Request:** campaign::request::Resolve. **Response:** `200 OK`, `common::Envelope<()>` as `application/json`.

This independent example assumes generation 1 is currently `unknown`. A mismatched generation or incompatible state prevents the update.

Request body (`request.json`):

```json
{
  "generation": 1,
  "action": "confirm_accepted"
}
```

```sh
curl --request POST "$RUPHISH_URL/api/campaigns/1/delivery/1" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header "Content-Type: application/json" \
  --data-binary @request.json
```

Example response:

```json
{
  "success": true,
  "message": "Delivery updated",
  "data": null
}
```

| Attribute | Type | Contract |
| --- | --- | --- |
| generation | integer | Generation returned by the delivery row you reviewed. |
| action | string | `confirm_accepted` changes unknown to accepted; `resend_unknown` changes unknown to ready; `retry` changes failed or blocked to ready. |

`resend_unknown` can cause a duplicate email if the previous SMTP transaction was accepted. Select it only after resolving that uncertainty with the mail service.
