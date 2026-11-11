---
title: Sending profiles
description: Sending profiles endpoints, request fields, response objects, and revision handling.
sidebar:
  order: 14
---

Sending profiles are shared installation resources. Reads require `view_objects`; writes require `modify_objects`. Paths use the numeric resource ID.

## Request fields

| Attribute | Type | Contract |
| --- | --- | --- |
| id | integer | Omit on creation. For replacement, supply the same ID as the URL. |
| name | string | Required profile name. |
| interface_type | string | Interface label; use `SMTP`. |
| host | string | SMTP hostname with optional port, such as `smtp.example.com:587`. |
| from_address | string | SMTP envelope sender and default visible From address. |
| username | string | SMTP authentication username; empty when authentication is not configured. |
| password | string or null | Write-only secret. On update, omission or null preserves the saved password, an empty string clears it, and a nonempty string replaces it. |
| ignore_cert_errors | boolean | Whether to bypass certificate verification. Defaults to false. |
| headers | Header[] | Additional message headers. Each item has string `key` and `value`; CR, LF, and NUL are rejected, and the key must not be empty. |

PUT replaces the editable fields; it is not a partial update. Omitted strings default to empty, arrays to empty arrays, and booleans to false. Unknown JSON fields are ignored. Send only writable fields.

## Response fields

Responses add the [resource metadata](conventions.md#resource-metadata) to the content fields above. Passwords are never returned.

| Attribute | Type | Contract |
| --- | --- | --- |
| has_password | boolean | Whether a password is stored. There is no response `password` member. |
| username | string, optional | Returned when present in the stored profile. |

## POST `/api/smtp/`

Create a resource.

**Request:** resources::request::SendingProfile. **Response:** `201 Created`, `resource_details::Resource<resource_details::Profile>` as `application/json`.

The response includes `ETag: "1"`. Save the returned revision for subsequent replacement or deletion.

Request body (`request.json`):

```json
{
  "name": "Training sender",
  "interface_type": "SMTP",
  "host": "smtp.example.com:587",
  "from_address": "training@example.com",
  "ignore_cert_errors": false,
  "headers": [],
  "username": "training@example.com",
  "password": "example-password"
}
```

```sh
curl --request POST "$RUPHISH_URL/api/smtp/" \
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
}
```

## GET `/api/smtp/`

List the stored resources. The response is an array, including `[]` when empty.

**Request:** No body. **Response:** `200 OK`, `Vec<resource_details::Resource<resource_details::Profile>>` as `application/json`.

```sh
curl --request GET "$RUPHISH_URL/api/smtp/" \
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
  }
]
```

## GET `/api/smtp/{id}`

Read one resource.

**Request:** No body. **Response:** `200 OK`, `resource_details::Resource<resource_details::Profile>` as `application/json`.

The response includes an `ETag` containing the current quoted numeric revision, for example `ETag: "1"`.

```sh
curl --request GET "$RUPHISH_URL/api/smtp/1" \
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
}
```

## PUT `/api/smtp/{id}`

Replace the editable content at the current revision.

**Request:** resources::request::SendingProfile. **Response:** `200 OK`, `resource_details::Resource<resource_details::Profile>` as `application/json`.

Returns the new revision and `ETag: "2"`. A stale revision returns `412`; read again before deciding whether to replace newer content.

Request body (`request.json`):

```json
{
  "id": 1,
  "name": "Training sender",
  "interface_type": "SMTP",
  "host": "smtp.example.com:587",
  "from_address": "training@example.com",
  "ignore_cert_errors": false,
  "headers": [],
  "username": "training@example.com",
  "password": "example-password"
}
```

```sh
curl --request PUT "$RUPHISH_URL/api/smtp/1" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header 'If-Match: "1"' \
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
  "revision": 2,
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
}
```

## GET `/api/smtp/summary`

List resource metadata without authored content.

**Request:** No body. **Response:** `200 OK`, `resources::ResourceSummaries` as `application/json`.

```sh
curl --request GET "$RUPHISH_URL/api/smtp/summary" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

Example response:

```json
{
  "total": 1,
  "items": [
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
      "name": "Training sender",
      "modified_date": "2026-11-03T12:00:00Z",
      "active_campaigns": 0
    }
  ]
}
```

| Attribute | Type | Contract |
| --- | --- | --- |
| total | integer | Number of returned resources. |
| items | resources::ResourceSummary[] | Resource metadata plus `active_campaigns`, the active campaign reference count. |

## DELETE `/api/smtp/{id}`

Delete the resource at the specified revision.

**Request:** No body. **Response:** `200 OK`, `common::Envelope<()>` as `application/json`.

```sh
curl --request DELETE "$RUPHISH_URL/api/smtp/1" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header 'If-Match: "2"'
```

Example response:

```json
{
  "success": true,
  "message": "SMTP Deleted Successfully",
  "data": null
}
```

## Related operations

[Test SMTP connectivity or send a test message](imports-previews-and-test-sends.md). Saving a profile does not send mail or verify account ownership.

## Saved profile tests

### GET `/api/smtp/{id}/tests`

Requires permission to view resources. Returns `profile_tests::History`: the current sending-configuration `generation`, company-policy `required` flag, derived `eligible` state, optional `active` attempt, latest completed `connection` and `email` attempts, the latest ten completed attempts in `history`, and batched current/deleted `operators`. Missing profiles return `404`. Reading also finalizes expired attempts conservatively; it never resubmits them.

Attempts contain `id`, `generation`, `kind`, `operator`, nullable `email`, `started_at`, nullable `finished_at`, `outcome`, safe `code`, and nullable numeric `smtp_code`. Times are UTC Unix seconds. Outcomes are `running`, `submitting`, `passed`, `accepted`, `failed`, `interrupted`, and `unknown`. A connection check produces no email. Only an `accepted` fixed-content email test on the current generation supplies qualifying campaign-launch evidence.

### POST `/api/smtp/{id}/tests`

Requires permission to modify resources and the normal API-key or browser session/CSRF authorization. Supply a unique 16–64 character ASCII alphanumeric/hyphen operation `id` and the numeric profile `revision` you reviewed. Reuse the same identity and payload to reconcile a lost response; an existing operation is returned without repeating SMTP effects, including after profile edits. A changed payload under the same identity returns `412`. Overlapping operations for the same profile return `409` with `PROFILE_TEST_BUSY`; stale revisions return `412`.

Connection-only request:

```json
{
  "id": "33f2d178-6db4-45de-b2d0-40e71dd76543",
  "revision": 1,
  "kind": "connection",
  "email": null
}
```

Fixed-content email request:

```json
{
  "id": "fa3735e4-b139-4336-8e02-93912bb08dc1",
  "revision": 1,
  "kind": "email",
  "email": "operator@example.test"
}
```

The recipient is limited to 320 UTF-8 bytes. Invalid input returns `400` before a test attempt is recorded. SMTP results return `200` with the persisted attempt, including failed or unknown outcomes; inspect `outcome` instead of treating HTTP success as SMTP acceptance. The message has subject `Ruphish sending profile test`, fixed plain-text content, and no tracking links or preview identity. Template and campaign testing remain separate operations and do not qualify the profile.

Execution records its submission boundary before transmitting content. Expired checks become `interrupted`; submissions whose acceptance cannot be established become `unknown`. A later failed, interrupted, or unknown test revokes earlier readiness. Profile tests do not automatically retry, expire successful evidence, or create campaign events. Sending-configuration changes invalidate evidence through a new generation; metadata-only changes do not. The optional [company policy](company-settings.md) is rechecked at campaign launch and does not pause already-launched campaigns.
