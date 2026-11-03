---
title: Domains
description: Origin registration, eligibility, DNS verification, and domain response fields.
sidebar:
  order: 15
---

Domains register participant-facing origins. Reads require `view_objects`; changes and verification require `modify_objects`. The body limit is 8 KiB.

## Request fields

| Attribute | Type | Contract |
| --- | --- | --- |
| `name` | `string` | Operator-visible display name. |
| `origin` | `string` | Canonical public HTTP(S) origin. |
| `enabled` | `boolean` | Explicit operational enablement. |
| `revision` | `integer or null` | Optimistic concurrency revision. |

Create without `revision`; updates require the current numeric body revision. Unknown attributes are rejected. An origin is an HTTP(S) scheme, host, and optional port, without a page path, query, credentials, or fragment. The origin cannot be changed after creation.

## Response fields

| Attribute | Type | Contract |
| --- | --- | --- |
| `verification` | `Verification` | Effective policy, separate from historical observations. |
| `id` | `integer` | Persistent resource identifier. |
| `name` | `string` | Operator-visible display name. |
| `origin` | `string` | Canonical public HTTP(S) origin. |
| `hostname` | `string` | Canonical ASCII DNS name or IP address without IPv6 brackets. |
| `enabled` | `boolean` | Explicit operational enablement. |
| `revision` | `integer` | Optimistic concurrency revision. |
| `txt_name` | `string` | DNS TXT record name to publish; empty for IP origins. |
| `txt_value` | `string` | Exact DNS TXT record value to publish; empty for IP origins. |
| `challenge_expires` | `integer` | Challenge expiry as UTC Unix seconds. |
| `checked_at` | `integer` | Last completed check as UTC Unix seconds. |
| `verified_until` | `integer` | Expiry of the last complete successful verification. |
| `control` | `boolean` | Latest DNS control observation. |
| `dns` | `boolean` | Latest permitted resolution observation. |
| `routing` | `boolean` | Latest instance routing observation. |
| `tls` | `boolean` | Latest HTTPS observation. |
| `code` | `string` | Closed operational result code. |
| `status` | `string` | Server-derived verification validity, independent of enabled state. |
| `eligible` | `boolean` | Server-derived availability for campaign selection. |
| `checking` | `boolean` | Whether a persisted verification lease is active. |
| `active_check` | `VerificationRun or null` | Current queued or running attempt, independent of domain validity. |
| `history` | `VerificationRun[]` | Latest ten completed attempts, newest first; populated on detail responses. |
| `next_check_at` | `integer` | Next automatic or requested check, in UTC Unix seconds. |
| `campaigns` | `integer` | Number of referencing campaigns. |

Creator/editor fields follow [attribution](conventions.md#attribution). Unix timestamps use seconds; zero means no recorded deadline or check where applicable. Collection responses omit completed history by returning `history: []`; detail responses contain the latest ten completed runs.

### Effective verification

| Attribute | Type | Contract |
| --- | --- | --- |
| `required` | `boolean` | Whether the installation currently requires verification. |
| `status` | `string` | Effective verification state under current policy. |
| `expires_at` | `integer or null` | UTC Unix deadline, absent for unrestricted evidence validity. |
| `next_check_at` | `integer or null` | Next verification check, absent while verification is disabled. |

| `verification.status` | Meaning |
| --- | --- |
| not_required | Verification is disabled by company policy. |
| unsupported | Verification is required but the origin is HTTP or uses an IP address. |
| not_verified | No successful verification has been recorded. |
| verified | Current evidence satisfies verification. |
| expired | Previous verification has expired. |
| failed | A check completed without valid verification. |

Use `eligible` to determine whether the domain is currently selectable. The top-level `status` describes historical verification independently of enablement and current policy.

### Verification run

| Attribute | Type | Contract |
| --- | --- | --- |
| `id` | `integer` | Persistent attempt identity. |
| `state` | `string` | Queued, running, passed, failed or timed_out. |
| `scheduled_at` | `integer` | Earliest scheduled execution, in UTC Unix seconds. |
| `started_at` | `integer or null` | Actual worker start; absent for queued or imported historical checks. |
| `finished_at` | `integer or null` | Actual completion; absent while active. |
| `code` | `string` | Safe application-owned result code. |

## POST `/api/domains/`

Register an origin. This example requires domain verification to be disabled.

**Request:** domains::Edit. **Response:** `201 Created`, `domains::Domain` as `application/json`.

Returns `ETag: "1"`. Registration does not configure DNS, TLS, or a reverse proxy.

Request body (`request.json`):

```json
{
  "name": "Local campaign pages",
  "origin": "http://127.0.0.1:8081",
  "enabled": true
}
```

```sh
curl --request POST "$RUPHISH_URL/api/domains/" \
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
  "verification": {
    "required": false,
    "status": "not_required",
    "expires_at": null,
    "next_check_at": null
  },
  "id": 1,
  "name": "Local campaign pages",
  "origin": "http://127.0.0.1:8081",
  "hostname": "127.0.0.1",
  "enabled": true,
  "revision": 1,
  "txt_name": "",
  "txt_value": "",
  "challenge_expires": 1794312000,
  "checked_at": 0,
  "verified_until": 0,
  "control": false,
  "dns": false,
  "routing": false,
  "tls": false,
  "code": "",
  "status": "not_verified",
  "eligible": true,
  "checking": false,
  "active_check": null,
  "history": [],
  "next_check_at": 0,
  "campaigns": 0
}
```

## GET `/api/domains/`

List registered origins.

**Request:** No body. **Response:** `200 OK`, `Vec<domains::Domain>` as `application/json`.

```sh
curl --request GET "$RUPHISH_URL/api/domains/" \
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
    "verification": {
      "required": false,
      "status": "not_required",
      "expires_at": null,
      "next_check_at": null
    },
    "id": 1,
    "name": "Local campaign pages",
    "origin": "http://127.0.0.1:8081",
    "hostname": "127.0.0.1",
    "enabled": true,
    "revision": 1,
    "txt_name": "",
    "txt_value": "",
    "challenge_expires": 1794312000,
    "checked_at": 0,
    "verified_until": 0,
    "control": false,
    "dns": false,
    "routing": false,
    "tls": false,
    "code": "",
    "status": "not_verified",
    "eligible": true,
    "checking": false,
    "active_check": null,
    "history": [],
    "next_check_at": 0,
    "campaigns": 0
  }
]
```

## GET `/api/domains/validated/`

List origins that are eligible under the current policy, including origins that do not require verification.

**Request:** No body. **Response:** `200 OK`, `Vec<domains::Domain>` as `application/json`.

```sh
curl --request GET "$RUPHISH_URL/api/domains/validated/" \
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
    "verification": {
      "required": false,
      "status": "not_required",
      "expires_at": null,
      "next_check_at": null
    },
    "id": 1,
    "name": "Local campaign pages",
    "origin": "http://127.0.0.1:8081",
    "hostname": "127.0.0.1",
    "enabled": true,
    "revision": 1,
    "txt_name": "",
    "txt_value": "",
    "challenge_expires": 1794312000,
    "checked_at": 0,
    "verified_until": 0,
    "control": false,
    "dns": false,
    "routing": false,
    "tls": false,
    "code": "",
    "status": "not_verified",
    "eligible": true,
    "checking": false,
    "active_check": null,
    "history": [],
    "next_check_at": 0,
    "campaigns": 0
  }
]
```

## GET `/api/domains/{id}`

Read the current policy result and verification history.

**Request:** No body. **Response:** `200 OK`, `domains::Domain` as `application/json`.

Returns the current revision in `ETag`, for example `"1"`.

```sh
curl --request GET "$RUPHISH_URL/api/domains/1" \
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
  "verification": {
    "required": false,
    "status": "not_required",
    "expires_at": null,
    "next_check_at": null
  },
  "id": 1,
  "name": "Local campaign pages",
  "origin": "http://127.0.0.1:8081",
  "hostname": "127.0.0.1",
  "enabled": true,
  "revision": 1,
  "txt_name": "",
  "txt_value": "",
  "challenge_expires": 1794312000,
  "checked_at": 0,
  "verified_until": 0,
  "control": false,
  "dns": false,
  "routing": false,
  "tls": false,
  "code": "",
  "status": "not_verified",
  "eligible": true,
  "checking": false,
  "active_check": null,
  "history": [],
  "next_check_at": 0,
  "campaigns": 0
}
```

## PUT `/api/domains/{id}`

Update the name or enablement at the supplied body revision.

**Request:** domains::Edit. **Response:** `200 OK`, `domains::Domain` as `application/json`.

Returns the new revision and `ETag: "2"`. `If-Match` does not replace the body revision for this operation.

Request body (`request.json`):

```json
{
  "name": "Local campaign pages",
  "origin": "http://127.0.0.1:8081",
  "enabled": true,
  "revision": 1
}
```

```sh
curl --request PUT "$RUPHISH_URL/api/domains/1" \
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
  "verification": {
    "required": false,
    "status": "not_required",
    "expires_at": null,
    "next_check_at": null
  },
  "id": 1,
  "name": "Local campaign pages",
  "origin": "http://127.0.0.1:8081",
  "hostname": "127.0.0.1",
  "enabled": true,
  "revision": 2,
  "txt_name": "",
  "txt_value": "",
  "challenge_expires": 1794312000,
  "checked_at": 0,
  "verified_until": 0,
  "control": false,
  "dns": false,
  "routing": false,
  "tls": false,
  "code": "",
  "status": "not_verified",
  "eligible": true,
  "checking": false,
  "active_check": null,
  "history": [],
  "next_check_at": 0,
  "campaigns": 0
}
```

## POST `/api/domains/{id}/verify`

Queue verification for an HTTPS DNS origin.

**Request:** No body. **Response:** `202 Accepted`, `domains::Domain` as `application/json`.

This independent example assumes domain 1 is `https://training.example.com`. Publish the returned TXT record and route that origin to the participant listener before requesting a check. Poll GET to observe completion; `202` acknowledges scheduling, not verification success.

```sh
curl --request POST "$RUPHISH_URL/api/domains/1/verify" \
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
  "verification": {
    "required": true,
    "status": "not_verified",
    "expires_at": null,
    "next_check_at": 1793707200
  },
  "id": 1,
  "name": "Training pages",
  "origin": "https://training.example.com",
  "hostname": "training.example.com",
  "enabled": true,
  "revision": 1,
  "txt_name": "_ruphish-verify.training.example.com",
  "txt_value": "ruphish-verification=example-challenge",
  "challenge_expires": 1794312000,
  "checked_at": 0,
  "verified_until": 0,
  "control": false,
  "dns": false,
  "routing": false,
  "tls": false,
  "code": "",
  "status": "not_verified",
  "eligible": false,
  "checking": false,
  "active_check": {
    "id": 1,
    "state": "queued",
    "scheduled_at": 1793707200,
    "started_at": null,
    "finished_at": null,
    "code": ""
  },
  "history": [],
  "next_check_at": 1793707200,
  "campaigns": 0
}
```

## DELETE `/api/domains/{id}`

Delete an unreferenced domain at its current revision.

**Request:** No body. **Response:** `200 OK`, `common::Envelope<()>` as `application/json`.

Any retained campaign reference blocks deletion, including completed campaigns.

```sh
curl --request DELETE "$RUPHISH_URL/api/domains/1" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header 'If-Match: "2"'
```

Example response:

```json
{
  "success": true,
  "message": "Domain deleted",
  "data": null
}
```
