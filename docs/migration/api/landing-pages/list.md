---
title: List landing pages
description: Migrate GET /api/pages/ to the current Ruphish contract.
sidebar:
  order: 140
prev: {"label": "Choose endpoint comparisons", "link": "/docs/migration/api/"}
next: {"label": "Verify and cut over", "link": "/docs/migration/verification-and-rollback/"}
---

Adapt this operation to shared resources and the current landing page contract.

| | Gophish 0.12.1 | Ruphish |
| --- | --- | --- |
| Operation | `GET /api/pages/` | `GET /api/pages/` |
| Success | `200 OK` | `200 OK` |
| Response type | `[]models.Page` | `Vec<resource_details::Resource<resource_details::Page>>` |

Use a Bearer API key. The [common changes](../common-changes.md) explain shared access, error handling, metadata, and example conventions.

## Required changes

| Attribute / behavior | Gophish 0.12.1 | Ruphish / client change |
| --- | --- | --- |
| `id`, `name` | Integer identity and string display name. | Types remain. IDs select shared objects; names are no longer scoped to the current user. |
| `modified_date` | RFC 3339 string; unknown dates can appear as Go zero time. | String or null. Handle unknown dates explicitly. |
| `revision` | Absent. | Integer, added. Detail reads and successful writes provide a quoted numeric `ETag`. |
| `created_at`, `edited_at` | Absent. | Nullable timestamp strings; server-assigned. |
| `created_by`, `edited_by` | Absent. | Nullable account references with integer `id`, nullable string `username`, and boolean `deleted`; attribution does not restrict access. |
| `html`, `redirect_url` | Strings. | Same names/types; authored HTML is handled by Ruphish rendering and submission rules. |
| `capture_credentials`, `capture_passwords` | Boolean write and response fields. | Removed from the contract. Remove them from writes and response models; entered values are not collected. |
| `warnings` | Absent. | Array on detail/save responses; not on collection items. See the landing-page reference for diagnostic fields. |

## Gophish request

`GET /api/pages/`

No request body.

## Gophish response

```json
[
  {
    "id": 1,
    "name": "Training page",
    "modified_date": "2026-11-03T12:00:00Z",
    "html": "<html><head></head><body><h1>Training page</h1></body></html>",
    "redirect_url": "",
    "capture_credentials": false,
    "capture_passwords": false
  }
]
```

## Ruphish request

No request body.

```sh
curl --request GET "$RUPHISH_URL/api/pages/" \
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
    "name": "Training page",
    "modified_date": "2026-11-03T12:00:00Z",
    "html": "<h1>Training page</h1>",
    "redirect_url": ""
  }
]
```

## Behavior and verification

Reads require `view_objects`; writes require `modify_objects`. Resources are shared across the installation. Use IDs to disambiguate imported resources with duplicate names.

Check the returned fields against your client model, including nullable metadata and an empty collection where applicable.

Current contract: [List landing pages](../../../api/landing-pages.md). Comparison source: [Gophish handler at the pinned version](https://github.com/gophish/gophish/blob/95618469799295e2c0fec980805a2dfbb818816b/controllers/api/page.go).
