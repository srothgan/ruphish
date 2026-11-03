---
title: Read an email template
description: Migrate GET /api/templates/{id} to the current Ruphish contract.
sidebar:
  order: 121
prev: {"label": "Choose endpoint comparisons", "link": "/docs/migration/api/"}
next: {"label": "Verify and cut over", "link": "/docs/migration/verification-and-rollback/"}
---

Adapt this operation to shared resources and the current email template contract.

| | Gophish 0.12.1 | Ruphish |
| --- | --- | --- |
| Operation | `GET /api/templates/{id}` | `GET /api/templates/{id}` |
| Success | `200 OK` | `200 OK` |
| Response type | `models.Template` | `resource_details::Resource<resource_details::Template>` |

Use a Bearer API key. The [common changes](../common-contract-changes.md) explain shared access, error handling, metadata, and example conventions.

## Required changes

| Attribute / behavior | Gophish 0.12.1 | Ruphish / client change |
| --- | --- | --- |
| `id`, `name` | Integer identity and string display name. | Types remain. IDs select shared objects; names are no longer scoped to the current user. |
| `modified_date` | RFC 3339 string; unknown dates can appear as Go zero time. | String or null. Handle unknown dates explicitly. |
| `revision` | Absent. | Integer, added. Detail reads and successful writes provide a quoted numeric `ETag`. |
| `created_at`, `edited_at` | Absent. | Nullable timestamp strings; server-assigned. |
| `created_by`, `edited_by` | Absent. | Nullable account references with integer `id`, nullable string `username`, and boolean `deleted`; attribution does not restrict access. |
| `subject`, `text`, `html`, `envelope_sender` | Strings. | Same names/types. `envelope_sender` is a visible From override, not a new JSON field name. |
| `attachments` | Array of `name`, `type`, `content` strings. | Same fields; content is base64. Send an empty array to replace with no attachments. |

## Gophish request

`GET /api/templates/{id}`

No request body.

## Gophish response

```json
{
  "id": 1,
  "name": "Training message",
  "modified_date": "2026-11-03T12:00:00Z",
  "subject": "Training notification",
  "text": "Hello {{.FirstName}}",
  "html": "<p>Hello {{.FirstName}}</p>",
  "envelope_sender": "",
  "attachments": []
}
```

## Ruphish request

No request body.

```sh
curl --request GET "$RUPHISH_URL/api/templates/1" \
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
  "name": "Training message",
  "modified_date": "2026-11-03T12:00:00Z",
  "subject": "Training notification",
  "text": "Hello {{.FirstName}}",
  "html": "<p>Hello {{.FirstName}}</p>",
  "envelope_sender": "",
  "attachments": []
}
```

## Behavior and verification

Reads require `view_objects`; writes require `modify_objects`. Resources are shared across the installation. Use IDs to disambiguate imported resources with duplicate names.

Check the returned fields against your client model, including nullable metadata and an empty collection where applicable.

Current contract: [Read an email template](../../../api/email-templates.md). Comparison source: [Gophish handler at the pinned version](https://github.com/gophish/gophish/blob/95618469799295e2c0fec980805a2dfbb818816b/controllers/api/template.go).
