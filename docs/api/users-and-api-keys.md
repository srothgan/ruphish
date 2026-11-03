---
title: Accounts and keys
description: Account administration and rotation of the authenticated API key.
sidebar:
  order: 41
---

Account endpoints return application accounts, including their API keys. List and create operations require system administration. Reading another account requires `modify_system`; reading your own account does not. Editing requires `modify_objects`, and editing another account additionally requires `modify_system`. Ordinary users cannot change their role, lock state, or password-setup requirement.

## Request fields

| Attribute | Type | Contract |
| --- | --- | --- |
| `username` | `string` | Exact login name. |
| `password` | `string` | New password, if provided. |
| `role` | `string` | Role slug. |
| `password_change_required` | `boolean` | Require password setup. |
| `account_locked` | `boolean` | Disable authentication. |

This is a full replacement input. Omitted strings default to empty and omitted flags to false; send the intended current values. Creation requires a password. On update an empty password preserves the existing password. Administrators can reset another account’s password; this operation rejects replacing your own password. Built-in role slugs are `admin` and `user`. Passwords are 8–1,024 UTF-8 bytes.

## Response fields

| Attribute | Type | Contract |
| --- | --- | --- |
| `id` | `integer` | Persistent identifier. |
| `username` | `string` | Exact login name. |
| `api_key` | `string` | Automation credential; returned only by authorized account endpoints. |
| `role` | `Role` | Role metadata. |
| `password_change_required` | `boolean` | Mandatory password setup. |
| `account_locked` | `boolean` | Account is disabled. |
| `last_login` | `string or null` | Most recent successful login. |

| Attribute | Type | Contract |
| --- | --- | --- |
| `slug` | `string` | Stable role name. |
| `name` | `string` | Display name. |
| `description` | `string` | Description. |

`last_login` is an RFC 3339 timestamp or null. Treat `api_key` as a credential; do not include account responses in application logs.

## POST `/api/users/`

Create an application account.

**Request:** auth::UserEdit. **Response:** `200 OK`, `auth::User` as `application/json`.

This endpoint returns `200 OK`, not `201`.

Request body (`request.json`):

```json
{
  "username": "automation",
  "password": "example-password",
  "role": "user",
  "password_change_required": false,
  "account_locked": false
}
```

```sh
curl --request POST "$RUPHISH_URL/api/users/" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header "Content-Type: application/json" \
  --data-binary @request.json
```

Example response:

```json
{
  "id": 2,
  "username": "automation",
  "api_key": "example-api-key",
  "role": {
    "slug": "user",
    "name": "User",
    "description": "User role with edit access to objects and campaigns"
  },
  "password_change_required": false,
  "account_locked": false,
  "last_login": null
}
```

## GET `/api/users/`

List accounts visible to an administrator.

**Request:** No body. **Response:** `200 OK`, `Vec<auth::User>` as `application/json`.

```sh
curl --request GET "$RUPHISH_URL/api/users/" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

Example response:

```json
[
  {
    "id": 2,
    "username": "automation",
    "api_key": "example-api-key",
    "role": {
      "slug": "user",
      "name": "User",
      "description": "User role with edit access to objects and campaigns"
    },
    "password_change_required": false,
    "account_locked": false,
    "last_login": null
  }
]
```

## GET `/api/users/{id}`

Read the specified account.

**Request:** No body. **Response:** `200 OK`, `auth::User` as `application/json`.

```sh
curl --request GET "$RUPHISH_URL/api/users/2" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

Example response:

```json
{
  "id": 2,
  "username": "automation",
  "api_key": "example-api-key",
  "role": {
    "slug": "user",
    "name": "User",
    "description": "User role with edit access to objects and campaigns"
  },
  "password_change_required": false,
  "account_locked": false,
  "last_login": null
}
```

## PUT `/api/users/{id}`

Replace account configuration while preserving its password.

**Request:** auth::UserEdit. **Response:** `200 OK`, `auth::User` as `application/json`.

Request body (`request.json`):

```json
{
  "username": "automation",
  "password": "",
  "role": "user",
  "password_change_required": false,
  "account_locked": false
}
```

```sh
curl --request PUT "$RUPHISH_URL/api/users/2" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header "Content-Type: application/json" \
  --data-binary @request.json
```

Example response:

```json
{
  "id": 2,
  "username": "automation",
  "api_key": "example-api-key",
  "role": {
    "slug": "user",
    "name": "User",
    "description": "User role with edit access to objects and campaigns"
  },
  "password_change_required": false,
  "account_locked": false,
  "last_login": null
}
```

## DELETE `/api/users/{id}`

Delete another account. Self-deletion and removal of the last available administrator are rejected.

**Request:** No body. **Response:** `200 OK`, `common::Envelope<()>` as `application/json`.

```sh
curl --request DELETE "$RUPHISH_URL/api/users/2" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

Example response:

```json
{
  "success": true,
  "message": "User deleted Successfully!",
  "data": null
}
```

## POST `/api/reset`

Rotate the authenticated account’s API key. **Request:** empty URL-encoded form, with Bearer authentication. **Response:** `200 OK`, `common::Envelope<String>`. This example is independent of account deletion above.

```sh
curl --request POST "$RUPHISH_URL/api/reset" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header "Content-Type: application/x-www-form-urlencoded" \
  --data ""
```

Example response:

```json
{
  "success": true,
  "message": "API Key successfully reset",
  "data": "example-replacement-api-key"
}
```

`data` is the new key. Store it and replace the old credential in your integration. The previous key stops working immediately; retrying with that old key will fail authentication.
