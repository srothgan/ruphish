---
title: Update a group
description: Migrate PUT /api/groups/{id} to the current Ruphish contract.
sidebar:
  order: 105
prev: {"label": "Choose endpoint comparisons", "link": "/docs/migration/api/"}
next: {"label": "Verify and cut over", "link": "/docs/migration/verification-and-rollback/"}
---

Read the current revision before replacing the group. A blind Gophish write no longer succeeds.

| | Gophish 0.12.1 | Ruphish |
| --- | --- | --- |
| Operation | `PUT /api/groups/{id}` | `PUT /api/groups/{id}` |
| Success | `200 OK` | `200 OK` |
| Response type | `models.Group` | `resources::details::Resource<resources::details::Group>` |

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
| `targets` | Array of recipients with `email`, `first_name`, `last_name`, `position` strings. | Same names/types. No stored recipient ID is exposed in this resource. |
| Writable fields | Go model decoded from JSON. | Send editable fields only. These resource requests ignore unknown properties; obsolete properties have no effect. |
| Body `id` | Must match the route ID. | Still required to match. A mismatch is 500 for groups and 400 for templates, pages, and SMTP. |

## Gophish request

`PUT /api/groups/{id}`

```json
{
  "id": 1,
  "name": "Training recipients",
  "targets": [
    {
      "email": "ada@example.com",
      "first_name": "Ada",
      "last_name": "Lovelace",
      "position": "Engineer"
    }
  ]
}
```

## Gophish response

```json
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
```

## Ruphish request

Request type: `resources::request::Group`. Save this body as `request.json`:

```json
{
  "id": 1,
  "name": "Training recipients",
  "targets": [
    {
      "email": "ada@example.com",
      "first_name": "Ada",
      "last_name": "Lovelace",
      "position": "Engineer"
    }
  ]
}
```

```sh
curl --request PUT "$RUPHISH_URL/api/groups/1" \
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
```

## Behavior and verification

Reads require `view_objects`; writes require `modify_objects`. Resources are shared across the installation. Use IDs to disambiguate imported resources with duplicate names. Send all editable fields you intend to keep; omitted strings, arrays, and booleans become their defaults. A stale revision must trigger a reload and review, not an unconditional retry.

In an isolated test, read the object, perform a conditional write, then verify that a stale revision is handled without overwriting newer work.

Current contract: [Update a group](../../../api/groups.md). Comparison source: [Gophish handler at the pinned version](https://github.com/gophish/gophish/blob/95618469799295e2c0fec980805a2dfbb818816b/controllers/api/group.go).
