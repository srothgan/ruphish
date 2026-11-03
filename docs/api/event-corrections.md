---
title: Event corrections
description: Typed additions, replacements, and deletions of recipient observations.
sidebar:
  order: 24
---

Event corrections require `modify_objects` and use numeric campaign and recipient-row IDs. The request limit is 4 KiB. The campaign must contain the recipient; event time must be within the campaign’s permitted observation interval and must not be in the future.

## Request fields

| Attribute | Type | Contract |
| --- | --- | --- |
| `request_id` | `string` | Caller-generated identity used to deduplicate creation retries. |
| `event_id` | `integer or null` | Target identity; null only for an addition. |
| `revision` | `integer` | Reviewed row version; zero for a new event. |
| `action` | `Action` | The operation to perform. |
| `value` | `Value or null` | Required for add/edit and forbidden for delete. |

| Attribute | Type | Contract |
| --- | --- | --- |
| `time` | `string` | UTC instant with an explicit offset. |
| `kind` | `Kind` | Engagement or reporting event only. |
| `presence` | `Presence` | Optional presence indicators; applicable only to submissions. |

| Attribute | Type | Contract |
| --- | --- | --- |
| `username_present` | `boolean or null` | Explicit username presence, if known. |
| `password_present` | `boolean or null` | Explicit password presence, if known. |

`request_id` is 16–64 ASCII letters, digits, or hyphens. Reuse it for retry reconciliation of the same addition. Editable kinds are `email_opened`, `link_clicked`, `form_submitted`, and `email_reported`; transport and campaign lifecycle events cannot be authored through this endpoint. Presence flags are only meaningful for form submissions; use null for other kinds.

| Action | Required shape |
| --- | --- |
| add | `event_id: null`, `revision: 0`, and a complete `value`. |
| edit | Existing `event_id`, current event `revision`, and a complete replacement `value`. |
| delete | Existing `event_id`, current event `revision`, and `value: null`. |

## POST `/api/campaigns/{id}/recipients/{recipient_id}/events`

Add an operator observation to a recipient history.

**Request:** ruphish_domain::event_edit::Change. **Response:** `200 OK`, `ruphish_domain::event_edit::Outcome` as `application/json`.

This example assumes campaign 1 was active at the supplied timestamp. It is independent of the future-scheduled campaign examples.

Request body (`request.json`):

```json
{
  "request_id": "example-event-0001",
  "event_id": null,
  "revision": 0,
  "action": "add",
  "value": {
    "time": "2026-11-03T12:00:00Z",
    "kind": "link_clicked",
    "presence": {
      "username_present": null,
      "password_present": null
    }
  }
}
```

```sh
curl --request POST "$RUPHISH_URL/api/campaigns/1/recipients/1/events" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header "Content-Type: application/json" \
  --data-binary @request.json
```

Example response:

```json
{
  "id": 10,
  "event": {
    "client": null,
    "id": 10,
    "source": "operator",
    "revision": 1,
    "edited": false,
    "current": {
      "time": "2026-11-03T12:00:00Z",
      "kind": {
        "type": "known",
        "value": "link_clicked"
      },
      "presence": {
        "username_present": null,
        "password_present": null
      }
    }
  }
}
```

## Response fields

| Attribute | Type | Contract |
| --- | --- | --- |
| `id` | `integer` | Affected identity, including for deletion. |
| `event` | `Record or null` | Current row after create/update; null after deletion. |

| Attribute | Type | Contract |
| --- | --- | --- |
| `client` | `Info or null` | Immutable coarse request categories; absent for historical or non-client observations. |
| `id` | `integer` | Persistent event identity. |
| `source` | `string` | Origin stays unchanged when an event is edited. |
| `revision` | `integer` | Row version for rejecting concurrent edits; not an edited indicator. |
| `edited` | `boolean` | True after an operator has actually changed this event's values. |
| `current` | `Fact` | Current event values. |

| Attribute | Type | Contract |
| --- | --- | --- |
| `time` | `string` | Recorded occurrence instant; imported unparseable values remain unchanged. |
| `kind` | `RecordedKind` | Classified observation. |
| `presence` | `Presence` | Presence indicators only. |

### Edit request and response

```json
{
  "request_id": "example-event-0002",
  "event_id": 10,
  "revision": 1,
  "action": "edit",
  "value": {
    "time": "2026-11-03T12:01:00Z",
    "kind": "link_clicked",
    "presence": {
      "username_present": null,
      "password_present": null
    }
  }
}
```

```json
{
  "id": 10,
  "event": {
    "client": null,
    "id": 10,
    "source": "operator",
    "revision": 2,
    "edited": true,
    "current": {
      "time": "2026-11-03T12:01:00Z",
      "kind": {
        "type": "known",
        "value": "link_clicked"
      },
      "presence": {
        "username_present": null,
        "password_present": null
      }
    }
  }
}
```

### Delete request and response

```json
{
  "request_id": "example-event-0003",
  "event_id": 10,
  "revision": 2,
  "action": "delete",
  "value": null
}
```

```json
{"id":10,"event":null}
```

Send edit and delete bodies to the same POST endpoint. Corrections change reporting evidence; they do not send mail or create a participant interaction request.
