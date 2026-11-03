---
title: Company settings
description: Policy revisions, optional features, and bulk campaign closure.
sidebar:
  order: 40
---

All operations on this page require `modify_system`. The settings JSON body limit is 8 KiB.

## Settings fields

| Attribute | Type | Contract |
| --- | --- | --- |
| `revision` | `integer` | Optimistic concurrency revision. |
| `policy` | `Policy` | Complete installation policy. |
| `updated_at` | `integer` | Last policy change in UTC Unix seconds. |
| `updated_by` | `integer or null` | Administrator responsible for the last policy change. |


### Policy

| Attribute | Type | Contract |
| --- | --- | --- |
| `verification_enabled` | `boolean` | Whether verification is required and checks may run. |
| `verification_expires` | `boolean` | Whether successful evidence has a time limit. |
| `verification_validity_seconds` | `integer` | Successful evidence lifetime in seconds. |
| `verification_recheck_seconds` | `integer` | Automatic recheck interval in seconds. |
| `reporting_mailboxes_enabled` | `boolean` | Whether mailbox network operations are authorized. |
| `webhooks_enabled` | `boolean` | Whether webhook enqueueing and delivery are authorized. |

All policy fields are required; unknown fields are rejected. Validity must be 3,600–31,536,000 seconds and recheck interval 300–604,800 seconds. When expiry is enabled, the recheck interval plus its jitter allowance (`min(recheck / 10, 300)`) must fit within half the validity period.

## GET `/api/settings`

Read company policy and its revision.

**Request:** No body. **Response:** `200 OK`, `settings::Settings` as `application/json`.

Returns `ETag: "1"`.

```sh
curl --request GET "$RUPHISH_URL/api/settings" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

Example response:

```json
{
  "revision": 1,
  "policy": {
    "verification_enabled": false,
    "verification_expires": true,
    "verification_validity_seconds": 86400,
    "verification_recheck_seconds": 21600,
    "reporting_mailboxes_enabled": false,
    "webhooks_enabled": false
  },
  "updated_at": 1793707200,
  "updated_by": 1
}
```

## PUT `/api/settings`

Replace company policy at the current revision.

**Request:** settings::Edit. **Response:** `200 OK`, `settings::Settings` as `application/json`.

Returns the updated policy and `ETag: "2"`. A missing or invalid precondition returns `428`; a stale revision returns `412`.

Request body (`request.json`):

```json
{
  "policy": {
    "verification_enabled": false,
    "verification_expires": true,
    "verification_validity_seconds": 86400,
    "verification_recheck_seconds": 21600,
    "reporting_mailboxes_enabled": false,
    "webhooks_enabled": false
  }
}
```

```sh
curl --request PUT "$RUPHISH_URL/api/settings" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header 'If-Match: "1"' \
  --header "Content-Type: application/json" \
  --data-binary @request.json
```

Example response:

```json
{
  "revision": 2,
  "policy": {
    "verification_enabled": false,
    "verification_expires": true,
    "verification_validity_seconds": 86400,
    "verification_recheck_seconds": 21600,
    "reporting_mailboxes_enabled": false,
    "webhooks_enabled": false
  },
  "updated_at": 1793707200,
  "updated_by": 1
}
```

## POST `/api/campaigns/complete-all/preview`

Obtain a confirmation token for closing the current active campaign set.

**Request:** No body. **Response:** `200 OK`, `settings::ClosurePreview` as `application/json`.

```sh
curl --request POST "$RUPHISH_URL/api/campaigns/complete-all/preview" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

Example response:

```json
{
  "token": "example-closure-token",
  "expires_at": 1793707500,
  "campaigns": 2,
  "pending_deliveries": 15
}
```

| Attribute | Type | Contract |
| --- | --- | --- |
| `token` | `string` | Opaque actor-bound confirmation identity. |
| `expires_at` | `integer` | Confirmation deadline in UTC Unix seconds. |
| `campaigns` | `integer` | Number of affected launched campaigns across owners. |
| `pending_deliveries` | `integer` | Number of unsent deliveries at review time. |

## POST `/api/campaigns/complete-all`

Close the campaign set identified by the preview token.

**Request:** settings::CloseCampaigns. **Response:** `200 OK`, `settings::ClosureResult` as `application/json`.

Use the actual token from preview. It is bound to the caller and expires after five minutes. Confirmation is case-sensitive. If the active campaign set changes, request a new preview. A committed token can be retried to recover its recorded result.

Request body (`request.json`):

```json
{
  "token": "example-closure-token",
  "confirmation": "CLOSE ALL"
}
```

```sh
curl --request POST "$RUPHISH_URL/api/campaigns/complete-all" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header "Content-Type: application/json" \
  --data-binary @request.json
```

Example response:

```json
{
  "campaigns": 2,
  "cancelled_deliveries": 15,
  "completed_at": 1793707200
}
```

| Attribute | Type | Contract |
| --- | --- | --- |
| `campaigns` | `integer` | Number of affected launched campaigns across owners. |
| `cancelled_deliveries` | `integer` | Number of unsent deliveries cancelled by the operation. |
| `completed_at` | `integer` | Completion time in UTC Unix seconds. |
