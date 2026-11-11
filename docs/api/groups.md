---
title: Groups
description: Groups endpoints, request fields, response objects, and revision handling.
sidebar:
  order: 10
---

Groups are shared installation resources. Reads require `view_objects`; writes require `modify_objects`. Paths use the numeric resource ID.

## Request fields

| Attribute | Type | Contract |
| --- | --- | --- |
| id | integer | Omit on creation. For replacement, supply the same ID as the URL. |
| name | string | Required nonempty group name. |
| targets | Recipient[] | Audience, in stored order. Each recipient has `email`, `first_name`, `last_name`, and `position` strings. Email is required; the other strings default to empty. A group requires at least one recipient. |

PUT replaces the editable fields; it is not a partial update. Omitted strings default to empty, arrays to empty arrays, and booleans to false. Unknown JSON fields are ignored. Send only writable fields.

## Response fields

Responses combine the content fields above with [resource metadata](conventions.md#resource-metadata).

## POST `/api/groups/`

Create a resource.

**Request:** resources::request::Group. **Response:** `201 Created`, `resources::details::Resource<resources::details::Group>` as `application/json`.

The response includes `ETag: "1"`. Save the returned revision for subsequent replacement or deletion.

Request body (`request.json`):

```json
{
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
curl --request POST "$RUPHISH_URL/api/groups/" \
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

## GET `/api/groups/`

List the stored resources. The response is an array, including `[]` when empty.

**Request:** No body. **Response:** `200 OK`, `Vec<resources::details::Resource<resources::details::Group>>` as `application/json`.

```sh
curl --request GET "$RUPHISH_URL/api/groups/" \
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

## GET `/api/groups/{id}`

Read one resource.

**Request:** No body. **Response:** `200 OK`, `resources::details::Resource<resources::details::Group>` as `application/json`.

The response includes an `ETag` containing the current quoted numeric revision, for example `ETag: "1"`.

```sh
curl --request GET "$RUPHISH_URL/api/groups/1" \
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

## PUT `/api/groups/{id}`

Replace the editable content at the current revision.

**Request:** resources::request::Group. **Response:** `200 OK`, `resources::details::Resource<resources::details::Group>` as `application/json`.

Returns the new revision and `ETag: "2"`. A stale revision returns `412`; read again before deciding whether to replace newer content.

Request body (`request.json`):

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

## GET `/api/groups/summary`

List resource metadata without authored content.

**Request:** No body. **Response:** `200 OK`, `resources::GroupSummaries` as `application/json`.

```sh
curl --request GET "$RUPHISH_URL/api/groups/summary" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

Example response:

```json
{
  "total": 1,
  "groups": [
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
  ]
}
```

| Attribute | Type | Contract |
| --- | --- | --- |
| total | integer | Number of returned resources. |
| groups | resources::GroupSummary[] | Resource metadata plus `num_targets`, the recipient count. |

## GET `/api/groups/{id}/summary`

Read metadata and recipient count for one group.

**Request:** No body. **Response:** `200 OK`, `resources::GroupSummary` as `application/json`.

```sh
curl --request GET "$RUPHISH_URL/api/groups/1/summary" \
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
  "name": "Training recipients",
  "modified_date": "2026-11-03T12:00:00Z",
  "num_targets": 1
}
```

## DELETE `/api/groups/{id}`

Delete the resource at the specified revision.

**Request:** No body. **Response:** `200 OK`, `common::Envelope<()>` as `application/json`.

```sh
curl --request DELETE "$RUPHISH_URL/api/groups/1" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header 'If-Match: "2"'
```

Example response:

```json
{
  "success": true,
  "message": "Group deleted successfully!",
  "data": null
}
```

## Related operations

[Import and review CSV recipients](imports-previews-and-test-sends.md). Campaign preparation deduplicates the combined audience; see [campaigns](campaigns.md).
