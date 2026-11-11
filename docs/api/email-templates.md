---
title: Email templates
description: Email templates endpoints, request fields, response objects, and revision handling.
sidebar:
  order: 11
---

Email templates are shared installation resources. Reads require `view_objects`; writes require `modify_objects`. Paths use the numeric resource ID.

## Request fields

| Attribute | Type | Contract |
| --- | --- | --- |
| id | integer | Omit on creation. For replacement, supply the same ID as the URL. |
| name | string | Required template name. |
| subject | string | Subject template. |
| text | string | Plain-text body template. |
| html | string | HTML body template. At least one of `text` and `html` must contain content. |
| envelope_sender | string | Visible From override. Despite its name, this does not replace the SMTP envelope sender supplied by the sending profile. |
| attachments | Attachment[] | Each item contains `name` (filename), `type` (MIME type), and `content` (base64-encoded bytes). Empty array means no attachments. |

PUT replaces the editable fields; it is not a partial update. Omitted strings default to empty, arrays to empty arrays, and booleans to false. Unknown JSON fields are ignored. Send only writable fields.

## Response fields

Responses combine the content fields above with [resource metadata](conventions.md#resource-metadata).

## POST `/api/templates/`

Create a resource.

**Request:** resources::request::EmailTemplate. **Response:** `201 Created`, `resources::details::Resource<resources::details::Template>` as `application/json`.

The response includes `ETag: "1"`. Save the returned revision for subsequent replacement or deletion.

Request body (`request.json`):

```json
{
  "name": "Training message",
  "subject": "Training notification",
  "text": "Hello {{.FirstName}}",
  "html": "<p>Hello {{.FirstName}}</p>",
  "envelope_sender": "",
  "attachments": []
}
```

```sh
curl --request POST "$RUPHISH_URL/api/templates/" \
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
  "name": "Training message",
  "modified_date": "2026-11-03T12:00:00Z",
  "subject": "Training notification",
  "text": "Hello {{.FirstName}}",
  "html": "<p>Hello {{.FirstName}}</p>",
  "envelope_sender": "",
  "attachments": []
}
```

## GET `/api/templates/`

List the stored resources. The response is an array, including `[]` when empty.

**Request:** No body. **Response:** `200 OK`, `Vec<resources::details::Resource<resources::details::Template>>` as `application/json`.

```sh
curl --request GET "$RUPHISH_URL/api/templates/" \
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
    "name": "Training message",
    "modified_date": "2026-11-03T12:00:00Z",
    "subject": "Training notification",
    "text": "Hello {{.FirstName}}",
    "html": "<p>Hello {{.FirstName}}</p>",
    "envelope_sender": "",
    "attachments": []
  }
]
```

## GET `/api/templates/{id}`

Read one resource.

**Request:** No body. **Response:** `200 OK`, `resources::details::Resource<resources::details::Template>` as `application/json`.

The response includes an `ETag` containing the current quoted numeric revision, for example `ETag: "1"`.

```sh
curl --request GET "$RUPHISH_URL/api/templates/1" \
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
  "name": "Training message",
  "modified_date": "2026-11-03T12:00:00Z",
  "subject": "Training notification",
  "text": "Hello {{.FirstName}}",
  "html": "<p>Hello {{.FirstName}}</p>",
  "envelope_sender": "",
  "attachments": []
}
```

## PUT `/api/templates/{id}`

Replace the editable content at the current revision.

**Request:** resources::request::EmailTemplate. **Response:** `200 OK`, `resources::details::Resource<resources::details::Template>` as `application/json`.

Returns the new revision and `ETag: "2"`. A stale revision returns `412`; read again before deciding whether to replace newer content.

Request body (`request.json`):

```json
{
  "id": 1,
  "name": "Training message",
  "subject": "Training notification",
  "text": "Hello {{.FirstName}}",
  "html": "<p>Hello {{.FirstName}}</p>",
  "envelope_sender": "",
  "attachments": []
}
```

```sh
curl --request PUT "$RUPHISH_URL/api/templates/1" \
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
  "name": "Training message",
  "modified_date": "2026-11-03T12:00:00Z",
  "subject": "Training notification",
  "text": "Hello {{.FirstName}}",
  "html": "<p>Hello {{.FirstName}}</p>",
  "envelope_sender": "",
  "attachments": []
}
```

## GET `/api/templates/summary`

List resource metadata without authored content.

**Request:** No body. **Response:** `200 OK`, `resources::ResourceSummaries` as `application/json`.

```sh
curl --request GET "$RUPHISH_URL/api/templates/summary" \
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
      "name": "Training message",
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

## DELETE `/api/templates/{id}`

Delete the resource at the specified revision.

**Request:** No body. **Response:** `200 OK`, `common::Envelope<()>` as `application/json`.

```sh
curl --request DELETE "$RUPHISH_URL/api/templates/1" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header 'If-Match: "2"'
```

Example response:

```json
{
  "success": true,
  "message": "Template deleted successfully!",
  "data": null
}
```

## Related operations

[Import content, validate HTML, and render previews](imports-previews-and-test-sends.md).
