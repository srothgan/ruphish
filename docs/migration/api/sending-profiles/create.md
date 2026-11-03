---
title: Create a sending profile
description: Migrate POST /api/smtp/ to the current Ruphish contract.
sidebar:
  order: 162
prev: {"label": "Choose endpoint comparisons", "link": "/docs/migration/api/"}
next: {"label": "Verify and cut over", "link": "/docs/migration/verification-and-rollback/"}
---

Create a shared sending profile and retain the returned revision.

| | Gophish 0.12.1 | Ruphish |
| --- | --- | --- |
| Operation | `POST /api/smtp/` | `POST /api/smtp/` |
| Success | `201 Created` | `201 Created` |
| Response type | `models.SMTP` | `resource_details::Resource<resource_details::Profile>` |

Use a Bearer API key. The [common changes](../common-changes.md) explain shared access, error handling, metadata, and example conventions.

## Required changes

| Attribute / behavior | Gophish 0.12.1 | Ruphish / client change |
| --- | --- | --- |
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
| Name conflicts | Create checks names within the authenticated user. | A new name is checked across the resource family in the installation; a collision returns 409. |

## Gophish request

`POST /api/smtp/`

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

## Behavior and verification

Reads require `view_objects`; writes require `modify_objects`. Resources are shared across the installation. Use IDs to disambiguate imported resources with duplicate names. Send all editable fields you intend to keep; omitted strings, arrays, and booleans become their defaults. The password is the exception: omitted/null preserves the saved credential.

Check the returned fields against your client model, including nullable metadata and an empty collection where applicable.

Current contract: [Create a sending profile](../../../api/sending-profiles.md). Comparison source: [Gophish handler at the pinned version](https://github.com/gophish/gophish/blob/95618469799295e2c0fec980805a2dfbb818816b/controllers/api/smtp.go).
