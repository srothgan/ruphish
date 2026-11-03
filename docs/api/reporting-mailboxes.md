---
title: Reporting mailboxes
description: Reporting mailboxes configuration, action endpoints, and operational response fields.
sidebar:
  order: 17
---

These resources are shared across the installation. Reads require `view_objects`; configuration changes, deletion, and network actions require `modify_objects`. Administrators control the global feature switch through [settings](company-settings.md). While disabled, reads and deletion remain available; saves and network actions return `409 FEATURE_DISABLED`. The body limit is 16 KiB.

## Request fields

| Attribute | Type | Contract |
| --- | --- | --- |
| `name` | `string` | Operator-supplied display name. |
| `enabled` | `boolean` | Whether processing is enabled. |
| `host` | `string` | Service hostname. |
| `port` | `integer` | Service port. |
| `username` | `string` | Service login name. |
| `password` | `string or null` | Replacement password, 1–4,096 bytes; omission or null preserves it. |
| `tls` | `boolean` | Use implicit TLS; false requests STARTTLS. |
| `ignore_cert_errors` | `boolean` | Explicit certificate-verification exception. |
| `folder` | `string` | Monitored mailbox folder. |
| `restrict_domain` | `string` | Optional sender-domain filter. |
| `delete_reported` | `boolean` | Remove successfully correlated messages. |
| `frequency` | `integer` | Polling interval: 30–31,540,000 seconds. |
| `revision` | `integer or null` | Expected saved revision; absent only for creation. |

Unknown fields are rejected. Omit `revision` on creation; supply it on updates. `password` is required on creation. Omission or null preserves the stored secret on update; an empty replacement is rejected.

## Response fields

| Attribute | Type | Contract |
| --- | --- | --- |
| `id` | `integer` | Persistent identifier. |
| `name` | `string` | Operator-visible name. |
| `enabled` | `boolean` | Whether polling is enabled. |
| `host` | `string` | IMAP hostname. |
| `port` | `integer` | IMAP port. |
| `username` | `string` | Configured service account name. |
| `has_password` | `boolean` | Whether a service password is configured; never its value. |
| `tls` | `boolean` | Whether to use implicit TLS. |
| `ignore_cert_errors` | `boolean` | Explicitly skip server certificate chain and hostname verification; TLS is still required. |
| `folder` | `string` | Monitored mailbox folder. |
| `restrict_domain` | `string` | Optional sender-domain restriction. |
| `delete_reported` | `boolean` | Whether matched messages are removed. |
| `frequency` | `integer` | Polling interval: 30–31,540,000 seconds. |
| `revision` | `integer` | Optimistic concurrency revision. |
| `last_check` | `integer` | Last check time in UTC Unix seconds. |
| `last_success` | `integer` | Last successful check in UTC Unix seconds. |
| `next_check` | `integer` | Next scheduled check in UTC Unix seconds. |
| `running` | `boolean` | Whether a worker currently owns a check. |
| `code` | `string` | Application-owned outcome code. |
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

## POST `/api/reporting-mailboxes/`

Create the configuration.

**Request:** integrations::request::MailboxWrite. **Response:** `201 Created`, `integrations::Mailbox` as `application/json`.

Request body (`request.json`):

```json
{
  "name": "Reported messages",
  "enabled": true,
  "host": "imap.example.com",
  "port": 993,
  "username": "reports@example.com",
  "password": "example-password",
  "tls": true,
  "ignore_cert_errors": false,
  "folder": "INBOX",
  "restrict_domain": "",
  "delete_reported": false,
  "frequency": 300
}
```

```sh
curl --request POST "$RUPHISH_URL/api/reporting-mailboxes/" \
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
  "name": "Reported messages",
  "enabled": true,
  "host": "imap.example.com",
  "port": 993,
  "username": "reports@example.com",
  "tls": true,
  "ignore_cert_errors": false,
  "folder": "INBOX",
  "restrict_domain": "",
  "delete_reported": false,
  "frequency": 300,
  "revision": 1,
  "has_password": true,
  "history": [],
  "last_check": 0,
  "last_success": 0,
  "next_check": 0,
  "running": false,
  "code": ""
}
```

## GET `/api/reporting-mailboxes/`

List configurations.

**Request:** No body. **Response:** `200 OK`, `Vec<integrations::Mailbox>` as `application/json`.

```sh
curl --request GET "$RUPHISH_URL/api/reporting-mailboxes/" \
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
    "name": "Reported messages",
    "enabled": true,
    "host": "imap.example.com",
    "port": 993,
    "username": "reports@example.com",
    "tls": true,
    "ignore_cert_errors": false,
    "folder": "INBOX",
    "restrict_domain": "",
    "delete_reported": false,
    "frequency": 300,
    "revision": 1,
    "has_password": true,
    "history": [],
    "last_check": 0,
    "last_success": 0,
    "next_check": 0,
    "running": false,
    "code": ""
  }
]
```

## GET `/api/reporting-mailboxes/{id}`

Read configuration and recent operational history.

**Request:** No body. **Response:** `200 OK`, `integrations::Mailbox` as `application/json`.

```sh
curl --request GET "$RUPHISH_URL/api/reporting-mailboxes/1" \
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
  "name": "Reported messages",
  "enabled": true,
  "host": "imap.example.com",
  "port": 993,
  "username": "reports@example.com",
  "tls": true,
  "ignore_cert_errors": false,
  "folder": "INBOX",
  "restrict_domain": "",
  "delete_reported": false,
  "frequency": 300,
  "revision": 1,
  "has_password": true,
  "history": [],
  "last_check": 0,
  "last_success": 0,
  "next_check": 0,
  "running": false,
  "code": ""
}
```

## PUT `/api/reporting-mailboxes/{id}`

Replace configuration while preserving the saved secret.

**Request:** integrations::request::MailboxWrite. **Response:** `200 OK`, `integrations::Mailbox` as `application/json`.

Request body (`request.json`):

```json
{
  "name": "Reported messages",
  "enabled": true,
  "host": "imap.example.com",
  "port": 993,
  "username": "reports@example.com",
  "tls": true,
  "ignore_cert_errors": false,
  "folder": "INBOX",
  "restrict_domain": "",
  "delete_reported": false,
  "frequency": 300,
  "revision": 1
}
```

```sh
curl --request PUT "$RUPHISH_URL/api/reporting-mailboxes/1" \
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
  "name": "Reported messages",
  "enabled": true,
  "host": "imap.example.com",
  "port": 993,
  "username": "reports@example.com",
  "tls": true,
  "ignore_cert_errors": false,
  "folder": "INBOX",
  "restrict_domain": "",
  "delete_reported": false,
  "frequency": 300,
  "revision": 2,
  "has_password": true,
  "history": [],
  "last_check": 0,
  "last_success": 0,
  "next_check": 0,
  "running": false,
  "code": ""
}
```

## POST `/api/reporting-mailboxes/{id}/validate`

Validate the saved mailbox connection and credentials.

**Request:** No body. **Response:** `200 OK`, `integrations::Mailbox` as `application/json`.

Returns the resource after successful validation. Transport or validation failure is an error response, not a successful resource with `success: false`.

```sh
curl --request POST "$RUPHISH_URL/api/reporting-mailboxes/1/validate" \
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
  "name": "Reported messages",
  "enabled": true,
  "host": "imap.example.com",
  "port": 993,
  "username": "reports@example.com",
  "tls": true,
  "ignore_cert_errors": false,
  "folder": "INBOX",
  "restrict_domain": "",
  "delete_reported": false,
  "frequency": 300,
  "revision": 1,
  "has_password": true,
  "history": [],
  "last_check": 0,
  "last_success": 0,
  "next_check": 0,
  "running": false,
  "code": ""
}
```

## POST `/api/reporting-mailboxes/{id}/check`

Schedule processing of the saved mailbox.

**Request:** No body. **Response:** `202 Accepted`, `integrations::Mailbox` as `application/json`.

Scheduling does not mean that messages have been processed. Read the resource again for `running`, `code`, timestamps, and history.

```sh
curl --request POST "$RUPHISH_URL/api/reporting-mailboxes/1/check" \
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
  "name": "Reported messages",
  "enabled": true,
  "host": "imap.example.com",
  "port": 993,
  "username": "reports@example.com",
  "tls": true,
  "ignore_cert_errors": false,
  "folder": "INBOX",
  "restrict_domain": "",
  "delete_reported": false,
  "frequency": 300,
  "revision": 1,
  "has_password": true,
  "history": [],
  "last_check": 0,
  "last_success": 0,
  "next_check": 0,
  "running": false,
  "code": ""
}
```

## POST `/api/reporting-mailboxes/{id}/resume`

Resume a mailbox paused by a UIDVALIDITY change at the expected revision.

**Request:** common::Revision. **Response:** `200 OK`, `integrations::Mailbox` as `application/json`.

This independent example assumes the saved mailbox has code `mailbox_uidvalidity_changed`. Resume resets the UID checkpoint and increments the configuration revision; other states return a conflict.

Request body (`request.json`):

```json
{
  "revision": 1
}
```

```sh
curl --request POST "$RUPHISH_URL/api/reporting-mailboxes/1/resume" \
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
  "name": "Reported messages",
  "enabled": true,
  "host": "imap.example.com",
  "port": 993,
  "username": "reports@example.com",
  "tls": true,
  "ignore_cert_errors": false,
  "folder": "INBOX",
  "restrict_domain": "",
  "delete_reported": false,
  "frequency": 300,
  "revision": 2,
  "has_password": true,
  "history": [],
  "last_check": 0,
  "last_success": 0,
  "next_check": 0,
  "running": false,
  "code": ""
}
```

`tls: true` selects implicit TLS; false selects required STARTTLS. `ignore_cert_errors` changes certificate verification, not whether TLS is used. `delete_reported` applies only to successfully correlated reports.

## DELETE `/api/reporting-mailboxes/{id}`

Delete the configuration at its current revision.

**Request:** No body. **Response:** `200 OK`, `serde_json::Value` as `application/json`.

```sh
curl --request DELETE "$RUPHISH_URL/api/reporting-mailboxes/1" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header 'If-Match: "2"'
```

Example response:

```json
{
  "success": true
}
```
