---
title: Webhooks
description: Webhooks configuration, action endpoints, and operational response fields.
sidebar:
  order: 16
---

These resources are shared across the installation. Reads require `view_objects`; configuration changes, deletion, and network actions require `modify_objects`. Administrators control the global feature switch through [settings](company-settings.md). While disabled, reads and deletion remain available; saves and network actions return `409 FEATURE_DISABLED`. The body limit is 16 KiB.

## Request fields

| Attribute | Type | Contract |
| --- | --- | --- |
| `name` | `string` | Operator-supplied display name. |
| `url` | `string` | HTTP(S) destination URL, up to 1,000 bytes, without credentials or a fragment. |
| `secret` | `string or null` | Replacement signing secret, 1–1,024 bytes; omission or null preserves it. |
| `is_active` | `boolean` | Whether event delivery is enabled. |
| `revision` | `integer or null` | Expected saved revision; absent only for creation. |

Unknown fields are rejected. Omit `revision` on creation; supply it on updates. `secret` is required on creation. Omission or null preserves the stored secret on update; an empty replacement is rejected.

## Response fields

| Attribute | Type | Contract |
| --- | --- | --- |
| `id` | `integer` | Persistent identifier. |
| `name` | `string` | Operator-visible name. |
| `url` | `string` | Destination URL. |
| `is_active` | `boolean` | Whether event delivery is enabled. |
| `revision` | `integer` | Optimistic concurrency revision. |
| `has_secret` | `boolean` | Whether a signing secret is configured; never its value. |
| `history` | `Run[]` | Recent processing attempts. |

Responses also contain [attribution](conventions.md#attribution). Secrets are represented only by presence flags. History uses the following run object; nullable fields are returned explicitly as null.

### Run

| Attribute | Type | Contract |
| --- | --- | --- |
| `id` | `integer` | Persistent identifier. |
| `event_id` | `integer or null` | Associated webhook event, when applicable. |
| `attempt` | `integer or null` | Webhook delivery attempt number, when applicable. |
| `started` | `integer` | Attempt start in UTC Unix seconds. |
| `finished` | `integer or null` | Completion time; absent while in progress. |
| `status` | `integer or null` | HTTP response status, when applicable. |
| `code` | `string` | Application-owned outcome code. |
| `matched` | `integer` | Number of correlated reports. |
| `rejected` | `integer` | Number of rejected reports. |

## POST `/api/webhooks/`

Create the configuration.

**Request:** integrations::request::WebhookWrite. **Response:** `201 Created`, `integrations::Webhook` as `application/json`.

Request body (`request.json`):

```json
{
  "name": "Event receiver",
  "url": "https://events.example.com/ruphish",
  "secret": "example-signing-secret",
  "is_active": true
}
```

```sh
curl --request POST "$RUPHISH_URL/api/webhooks/" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header "Content-Type: application/json" \
  --data-binary @request.json
```

Example response:

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
  "id": 1,
  "name": "Event receiver",
  "url": "https://events.example.com/ruphish",
  "is_active": true,
  "revision": 1,
  "has_secret": true,
  "history": []
}
```

## GET `/api/webhooks/`

List configurations.

**Request:** No body. **Response:** `200 OK`, `Vec<integrations::Webhook>` as `application/json`.

```sh
curl --request GET "$RUPHISH_URL/api/webhooks/" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

Example response:

```json
[
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
    "id": 1,
    "name": "Event receiver",
    "url": "https://events.example.com/ruphish",
    "is_active": true,
    "revision": 1,
    "has_secret": true,
    "history": []
  }
]
```

## GET `/api/webhooks/{id}`

Read configuration and recent operational history.

**Request:** No body. **Response:** `200 OK`, `integrations::Webhook` as `application/json`.

```sh
curl --request GET "$RUPHISH_URL/api/webhooks/1" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

Example response:

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
  "id": 1,
  "name": "Event receiver",
  "url": "https://events.example.com/ruphish",
  "is_active": true,
  "revision": 1,
  "has_secret": true,
  "history": []
}
```

## PUT `/api/webhooks/{id}`

Replace configuration while preserving the saved secret.

**Request:** integrations::request::WebhookWrite. **Response:** `200 OK`, `integrations::Webhook` as `application/json`.

Request body (`request.json`):

```json
{
  "name": "Event receiver",
  "url": "https://events.example.com/ruphish",
  "is_active": true,
  "revision": 1
}
```

```sh
curl --request PUT "$RUPHISH_URL/api/webhooks/1" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header "Content-Type: application/json" \
  --data-binary @request.json
```

Example response:

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
  "id": 1,
  "name": "Event receiver",
  "url": "https://events.example.com/ruphish",
  "is_active": true,
  "revision": 2,
  "has_secret": true,
  "history": []
}
```

## POST `/api/webhooks/{id}/validate`

Validate the saved receiver with a signed HTTP request.

**Request:** No body. **Response:** `200 OK`, `integrations::Webhook` as `application/json`.

Returns the resource after successful validation. Transport or validation failure is an error response, not a successful resource with `success: false`.

```sh
curl --request POST "$RUPHISH_URL/api/webhooks/1/validate" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

Example response:

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
  "id": 1,
  "name": "Event receiver",
  "url": "https://events.example.com/ruphish",
  "is_active": true,
  "revision": 1,
  "has_secret": true,
  "history": []
}
```

## DELETE `/api/webhooks/{id}`

Delete the configuration at its current revision.

**Request:** No body. **Response:** `200 OK`, `serde_json::Value` as `application/json`.

```sh
curl --request DELETE "$RUPHISH_URL/api/webhooks/1" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header 'If-Match: "2"'
```

Example response:

```json
{
  "success": true
}
```

## Outgoing event request

Ruphish sends POST requests with `Content-Type: application/json`, `X-Ruphish-Event-ID`, and `X-Gophish-Signature`. The receiver URL is the saved `url`. Example event body:

```json
{
  "campaign_id": 1,
  "email": "ada@example.com",
  "time": "2026-11-03T12:00:00Z",
  "message": "Clicked Link",
  "details": "",
  "client": {
    "browser": "firefox",
    "os": "linux",
    "device": "desktop"
  }
}
```

| Attribute | Type | Contract |
| --- | --- | --- |
| campaign_id | integer | Campaign identity. |
| email | string | Campaign recipient address. |
| time | string | Event timestamp in RFC 3339 UTC. |
| message | string | Recorded event label, such as `Clicked Link`. |
| details | string | JSON-encoded sanitized details, or an empty string when none are retained; it is not an object in the outer payload. |
| client | object or null | Coarse browser, OS, and device categories; see [client observations](results-and-exports.md#client-observations). |

Validation sends this body instead of an event:

```json
{"success": true}
```

The receiver acknowledges acceptance with any 2xx status, for example `204 No Content` with no response body. Ruphish discards response bodies.

### Signatures and retries

`X-Gophish-Signature` is `sha256=` followed by lowercase hexadecimal HMAC-SHA256 over the exact request body bytes, using the configured secret. Verify it with a constant-time comparison before parsing or processing the event. Do not reserialize JSON before verification.

Deduplicate ordinary events by `X-Ruphish-Event-ID`, scoped to the source installation. Delivery may repeat and arrive out of order. A timeout can occur after the receiver has accepted the event. Persist acceptance before returning success.

Requests have a ten-second deadline and do not follow redirects. Network failures and HTTP 408, 429, 500, 502, 503, and 504 can be retried. Editing a configuration cancels queued work for its previous revision; an in-flight request can still finish.
