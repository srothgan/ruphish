---
title: Read a group summary
description: Migrate GET /api/groups/{id}/summary to the current Ruphish contract.
sidebar:
  order: 103
prev: {"label": "Choose endpoint comparisons", "link": "/docs/migration/api/"}
next: {"label": "Verify and cut over", "link": "/docs/migration/verification-and-rollback/"}
---

Update summary parsers for shared visibility, revisions, and attribution.

| | Gophish 0.12.1 | Ruphish |
| --- | --- | --- |
| Operation | `GET /api/groups/{id}/summary` | `GET /api/groups/{id}/summary` |
| Success | `200 OK` | `200 OK` |
| Response type | `models.GroupSummary` | `resources::GroupSummary` |

Use a Bearer API key. The [common changes](../common-contract-changes.md) explain shared access, error handling, metadata, and example conventions.

## Required changes

| Attribute / behavior | Gophish 0.12.1 | Ruphish / client change |
| --- | --- | --- |
| `id`, `name` | Integer identity and string display name. | Types remain. IDs select shared objects; names are no longer scoped to the current user. |
| `modified_date` | RFC 3339 string; unknown dates can appear as Go zero time. | String or null. Handle unknown dates explicitly. |
| `revision` | Absent. | Integer revision in the body. Summary responses do not provide an ETag; use the quoted revision for a conditional write. |
| `created_at`, `edited_at` | Absent. | Nullable timestamp strings; server-assigned. |
| `created_by`, `edited_by` | Absent. | Nullable account references with integer `id`, nullable string `username`, and boolean `deleted`; attribution does not restrict access. |
| `num_targets` | Integer recipient count. | Same meaning; summary omits recipient records. |

## Gophish request

`GET /api/groups/{id}/summary`

No request body.

## Gophish response

```json
{
  "id": 1,
  "name": "Training recipients",
  "modified_date": "2026-11-03T12:00:00Z",
  "num_targets": 1
}
```

## Ruphish request

No request body.

```sh
curl --request GET "$RUPHISH_URL/api/groups/1/summary" \
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
  "name": "Training recipients",
  "modified_date": "2026-11-03T12:00:00Z",
  "num_targets": 1
}
```

## Behavior and verification

Requires `view_objects`. A summary is a metadata projection, not the full audience.

Compare the count with the corresponding group detail and verify the client tolerates unknown historical attribution.

Current contract: [Read a group summary](../../../api/groups.md). Comparison source: [Gophish handler at the pinned version](https://github.com/gophish/gophish/blob/95618469799295e2c0fec980805a2dfbb818816b/controllers/api/group.go).
