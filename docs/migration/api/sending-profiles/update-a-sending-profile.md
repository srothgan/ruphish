---
title: Update a sending profile
description: Migrate PUT /api/smtp/{id} to the current Ruphish contract.
sidebar:
  order: 163
prev: {"label": "Choose endpoint comparisons", "link": "/docs/migration/api/"}
next: {"label": "Verify and cut over", "link": "/docs/migration/verification-and-rollback/"}
---

Read the current revision before replacing the sending profile. A blind Gophish write no longer succeeds.

| | Gophish 0.12.1 | Ruphish |
| --- | --- | --- |
| Operation | `PUT /api/smtp/{id}` | `PUT /api/smtp/{id}` |
| Success | `200 OK` | `200 OK` |
| Response type | `models.SMTP` | `resource_details::Resource<resource_details::Profile>` |

Use a Bearer API key. The [common changes](../common-contract-changes.md) explain shared access, error handling, metadata, and example conventions.

## Required changes

| Attribute / behavior | Gophish 0.12.1 | Ruphish / client change |
| --- | --- | --- |
| `If-Match` header | Not required. | Required quoted current revision, for example `"1"`; missing → 428, stale → 412. |
| `id`, `name` | Integer identity and string display name. | Types remain. IDs select shared objects; names are no longer scoped to the current user. |
| `modified_date` | RFC 3339 string; unknown dates can appear as Go zero time. | String or null. Handle unknown dates explicitly. |
| `revision` | Absent. | Integer, added. Detail reads and successful writes provide a quoted numeric `ETag`. |
| `created_at`, `edited_at` | Absent. | Nullable timestamp strings; server-assigned. |
| `created_by`, `edited_by` | Absent. | Nullable account references with integer `id`, nullable string `username`, and boolean `deleted`; attribution does not restrict access. |
| `interface_type`, `host`, `from_address` | Strings. | Same names/types. Configure the current SMTP transport requirements. |
| `username` | String, omitted when empty. | Still omitted when empty. |
| `password` | Credential could be returned in clear text when nonempty. | Write-only optional string; never returned. Omission/null on update preserves it; empty string clears it. |
| `has_password` | Absent. | Boolean indicating stored credential presence. Do not bind it to a password input. |
| `ignore_cert_errors`, `headers` | Boolean and array of string `key`/`value` objects. | Same fields. Preserving configuration does not prove a successful TLS/authentication handshake. |
| Writable fields | Go model decoded from JSON. | Send editable fields only. These resource requests ignore unknown properties; obsolete properties have no effect. |
| Body `id` | Must match the route ID. | Still required to match. A mismatch is 500 for groups and 400 for templates, pages, and SMTP. |

## Gophish request

`PUT /api/smtp/{id}`

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

## Gophish response

```json
{
  "id": 1,
  "name": "Training sender",
  "modified_date": "2026-11-03T12:00:00Z",
  "interface_type": "SMTP",
  "host": "smtp.example.com:587",
  "from_address": "training@example.com",
  "ignore_cert_errors": false,
  "headers": [],
  "username": "training@example.com",
  "password": "example-password"
}
```

## Ruphish request

Request type: `resources::request::SendingProfile`. Save this body as `request.json`:

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

## Ruphish response

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

## Behavior and verification

Reads require `view_objects`; writes require `modify_objects`. Resources are shared across the installation. Use IDs to disambiguate imported resources with duplicate names. Send all editable fields you intend to keep; omitted strings, arrays, and booleans become their defaults. The password is the exception: omitted/null preserves the saved credential. A stale revision must trigger a reload and review, not an unconditional retry.

In an isolated test, read the object, perform a conditional write, then verify that a stale revision is handled without overwriting newer work.

Current contract: [Update a sending profile](../../../api/sending-profiles.md). Comparison source: [Gophish handler at the pinned version](https://github.com/gophish/gophish/blob/95618469799295e2c0fec980805a2dfbb818816b/controllers/api/smtp.go).
