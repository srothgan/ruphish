---
title: Create a landing page
description: Migrate POST /api/pages/ to the current Ruphish contract.
sidebar:
  order: 142
prev: {"label": "Choose endpoint comparisons", "link": "/docs/migration/api/"}
next: {"label": "Verify and cut over", "link": "/docs/migration/verification-and-rollback/"}
---

Create a shared landing page and retain the returned revision.

| | Gophish 0.12.1 | Ruphish |
| --- | --- | --- |
| Operation | `POST /api/pages/` | `POST /api/pages/` |
| Success | `201 Created` | `201 Created` |
| Response type | `models.Page` | `resources::details::Resource<resources::details::Page>` |

Use a Bearer API key. The [common changes](../common-contract-changes.md) explain shared access, error handling, metadata, and example conventions.

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
| Writable fields | Go model decoded from JSON. | Send editable fields only. These resource requests ignore unknown properties; obsolete properties have no effect. |
| Name conflicts | Create checks names within the authenticated user. | A new name is checked across the resource family in the installation; a collision returns 409. |

## Gophish request

`POST /api/pages/`

```json
{
  "name": "Training page",
  "html": "<h1>Training page</h1>",
  "redirect_url": "",
  "capture_credentials": false,
  "capture_passwords": false
}
```

## Gophish response

```json
{
  "id": 1,
  "name": "Training page",
  "modified_date": "2026-11-03T12:00:00Z",
  "html": "<html><head></head><body><h1>Training page</h1></body></html>",
  "redirect_url": "",
  "capture_credentials": false,
  "capture_passwords": false
}
```

## Ruphish request

Request type: `resources::request::LandingPage`. Save this body as `request.json`:

```json
{
  "name": "Training page",
  "html": "<h1>Training page</h1>",
  "redirect_url": ""
}
```

```sh
curl --request POST "$RUPHISH_URL/api/pages/" \
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
  "name": "Training page",
  "modified_date": "2026-11-03T12:00:00Z",
  "html": "<h1>Training page</h1>",
  "redirect_url": "",
  "warnings": []
}
```

## Behavior and verification

Reads require `view_objects`; writes require `modify_objects`. Resources are shared across the installation. Use IDs to disambiguate imported resources with duplicate names. Send all editable fields you intend to keep; omitted strings, arrays, and booleans become their defaults.

Check the returned fields against your client model, including nullable metadata and an empty collection where applicable.

Current contract: [Create a landing page](../../../api/landing-pages.md). Comparison source: [Gophish handler at the pinned version](https://github.com/gophish/gophish/blob/95618469799295e2c0fec980805a2dfbb818816b/controllers/api/page.go).
