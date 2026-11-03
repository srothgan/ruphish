---
title: Landing pages
description: Landing pages endpoints, request fields, response objects, and revision handling.
sidebar:
  order: 12
---

Landing pages are shared installation resources. Reads require `view_objects`; writes require `modify_objects`. Paths use the numeric resource ID.

## Request fields

| Attribute | Type | Contract |
| --- | --- | --- |
| id | integer | Omit on creation. For replacement, supply the same ID as the URL. |
| name | string | Required page name. |
| html | string | Authored HTML. Ruphish prepares it for participant delivery; this response is the saved source. |
| redirect_url | string | Default post-submission redirect expression. Empty string means no external redirect. |

PUT replaces the editable fields; it is not a partial update. Omitted strings default to empty, arrays to empty arrays, and booleans to false. Unknown JSON fields are ignored. Send only writable fields.

## Response fields

Responses combine the content fields above with [resource metadata](conventions.md#resource-metadata).

| Attribute | Type | Contract |
| --- | --- | --- |
| warnings | PageWarning[], optional | Returned by detail and save operations; absent from collection items. An empty array means no warnings. Each warning has `code` and nullable `control`, a one-based control index. |

## POST `/api/pages/`

Create a resource.

**Request:** resources::request::LandingPage. **Response:** `201 Created`, `resource_details::Resource<resource_details::Page>` as `application/json`.

The response includes `ETag: "1"`. Save the returned revision for subsequent replacement or deletion.

Request body (`request.json`):

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
  "revision": 1,
  "id": 1,
  "name": "Training page",
  "modified_date": "2026-11-03T12:00:00Z",
  "html": "<h1>Training page</h1>",
  "redirect_url": "",
  "warnings": []
}
```

## GET `/api/pages/`

List the stored resources. The response is an array, including `[]` when empty.

**Request:** No body. **Response:** `200 OK`, `Vec<resource_details::Resource<resource_details::Page>>` as `application/json`.

```sh
curl --request GET "$RUPHISH_URL/api/pages/" \
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
    "revision": 1,
    "id": 1,
    "name": "Training page",
    "modified_date": "2026-11-03T12:00:00Z",
    "html": "<h1>Training page</h1>",
    "redirect_url": ""
  }
]
```

## GET `/api/pages/{id}`

Read one resource.

**Request:** No body. **Response:** `200 OK`, `resource_details::Resource<resource_details::Page>` as `application/json`.

The response includes an `ETag` containing the current quoted numeric revision, for example `ETag: "1"`.

```sh
curl --request GET "$RUPHISH_URL/api/pages/1" \
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
  "revision": 1,
  "id": 1,
  "name": "Training page",
  "modified_date": "2026-11-03T12:00:00Z",
  "html": "<h1>Training page</h1>",
  "redirect_url": "",
  "warnings": []
}
```

## PUT `/api/pages/{id}`

Replace the editable content at the current revision.

**Request:** resources::request::LandingPage. **Response:** `200 OK`, `resource_details::Resource<resource_details::Page>` as `application/json`.

Returns the new revision and `ETag: "2"`. A stale revision returns `412`; read again before deciding whether to replace newer content.

Request body (`request.json`):

```json
{
  "id": 1,
  "name": "Training page",
  "html": "<h1>Training page</h1>",
  "redirect_url": ""
}
```

```sh
curl --request PUT "$RUPHISH_URL/api/pages/1" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header 'If-Match: "1"' \
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
  "revision": 2,
  "id": 1,
  "name": "Training page",
  "modified_date": "2026-11-03T12:00:00Z",
  "html": "<h1>Training page</h1>",
  "redirect_url": "",
  "warnings": []
}
```

## GET `/api/pages/summary`

List resource metadata without authored content.

**Request:** No body. **Response:** `200 OK`, `resources::ResourceSummaries` as `application/json`.

```sh
curl --request GET "$RUPHISH_URL/api/pages/summary" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

Example response:

```json
{
  "total": 1,
  "items": [
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
      "active_campaigns": 0
    }
  ]
}
```

| Attribute | Type | Contract |
| --- | --- | --- |
| total | integer | Number of returned resources. |
| items | resources::ResourceSummary[] | Resource metadata plus `active_campaigns`, the active campaign reference count. |

## DELETE `/api/pages/{id}`

Delete the resource at the specified revision.

**Request:** No body. **Response:** `200 OK`, `common::Envelope<()>` as `application/json`.

```sh
curl --request DELETE "$RUPHISH_URL/api/pages/1" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header 'If-Match: "2"'
```

Example response:

```json
{
  "success": true,
  "message": "Page Deleted Successfully",
  "data": null
}
```

## Related operations

[Import content, validate HTML, and render previews](imports-and-previews.md).
