---
title: List groups
description: Migrate GET /api/groups/ to the current Ruphish contract.
sidebar:
  order: 100
prev: {"label": "Choose endpoint comparisons", "link": "/docs/migration/api/"}
next: {"label": "Verify and cut over", "link": "/docs/migration/verification-and-rollback/"}
---

Adapt this operation to shared resources and the current group contract.

| | Gophish 0.12.1 | Ruphish |
| --- | --- | --- |
| Operation | `GET /api/groups/` | `GET /api/groups/` |
| Success | `200 OK` | `200 OK` |
| Response type | `[]models.Group` | `Vec<resources::details::Resource<resources::details::Group>>` |

Use a Bearer API key. The [common changes](../common-contract-changes.md) explain shared access, error handling, metadata, and example conventions.

## Required changes

| Attribute / behavior | Gophish 0.12.1 | Ruphish / client change |
| --- | --- | --- |
| `id`, `name` | Integer identity and string display name. | Types remain. IDs select shared objects; names are no longer scoped to the current user. |
| `modified_date` | RFC 3339 string; unknown dates can appear as Go zero time. | String or null. Handle unknown dates explicitly. |
| `revision` | Absent. | Integer, added. Detail reads and successful writes provide a quoted numeric `ETag`. |
| `created_at`, `edited_at` | Absent. | Nullable timestamp strings; server-assigned. |
| `created_by`, `edited_by` | Absent. | Nullable account references with integer `id`, nullable string `username`, and boolean `deleted`; attribution does not restrict access. |
| `targets` | Array of recipients with `email`, `first_name`, `last_name`, `position` strings. | Same names/types. No stored recipient ID is exposed in this resource. |

## Gophish request

`GET /api/groups/`

No request body.

## Gophish response

```json
[
  {
    "id": 1,
    "name": "Training recipients",
    "modified_date": "2026-11-03T12:00:00Z",
    "targets": [
      {
        "email": "ada@example.com",
        "first_name": "Ada",
        "last_name": "Lovelace",
        "position": "Engineer"
      }
    ]
  }
]
```

## Ruphish request

No request body.

```sh
curl --request GET "$RUPHISH_URL/api/groups/" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

## Ruphish response

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
    "name": "Training recipients",
    "modified_date": "2026-11-03T12:00:00Z",
    "targets": [
      {
        "email": "ada@example.com",
        "first_name": "Ada",
        "last_name": "Lovelace",
        "position": "Engineer"
      }
    ]
  }
]
```

## Behavior and verification

Reads require `view_objects`; writes require `modify_objects`. Resources are shared across the installation. Use IDs to disambiguate imported resources with duplicate names.

Check the returned fields against your client model, including nullable metadata and an empty collection where applicable.

Current contract: [List groups](../../../api/groups.md). Comparison source: [Gophish handler at the pinned version](https://github.com/gophish/gophish/blob/95618469799295e2c0fec980805a2dfbb818816b/controllers/api/group.go).
