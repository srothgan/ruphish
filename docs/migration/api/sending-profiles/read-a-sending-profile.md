---
title: Read a sending profile
description: Migrate GET /api/smtp/{id} to the current Ruphish contract.
sidebar:
  order: 161
prev: {"label": "Choose endpoint comparisons", "link": "/docs/migration/api/"}
next: {"label": "Verify and cut over", "link": "/docs/migration/verification-and-rollback/"}
---

Adapt this operation to shared resources and the current sending profile contract.

| | Gophish 0.12.1 | Ruphish |
| --- | --- | --- |
| Operation | `GET /api/smtp/{id}` | `GET /api/smtp/{id}` |
| Success | `200 OK` | `200 OK` |
| Response type | `models.SMTP` | `resources::details::Resource<resources::details::Profile>` |

Use a Bearer API key. The [common changes](../common-contract-changes.md) explain shared access, error handling, metadata, and example conventions.

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

## Gophish request

`GET /api/smtp/{id}`

No request body.

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

No request body.

```sh
curl --request GET "$RUPHISH_URL/api/smtp/1" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
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

Reads require `view_objects`; writes require `modify_objects`. Resources are shared across the installation. Use IDs to disambiguate imported resources with duplicate names.

Check the returned fields against your client model, including nullable metadata and an empty collection where applicable.

Current contract: [Read a sending profile](../../../api/sending-profiles.md). Comparison source: [Gophish handler at the pinned version](https://github.com/gophish/gophish/blob/95618469799295e2c0fec980805a2dfbb818816b/controllers/api/smtp.go).
