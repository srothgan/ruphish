---
title: Update an account
description: Migrate PUT /api/users/{id} to the current Ruphish contract.
sidebar:
  order: 283
prev: {"label": "Choose endpoint comparisons", "link": "/docs/migration/api/"}
next: {"label": "Verify and cut over", "link": "/docs/migration/verification-and-rollback/"}
---

Update account handling for nullable login times and current privilege rules.

| | Gophish 0.12.1 | Ruphish |
| --- | --- | --- |
| Operation | `PUT /api/users/{id}` | `PUT /api/users/{id}` |
| Success | `200 OK` | `200 OK` |
| Response type | `models.User` | `auth::User` |

Use a Bearer API key. The [common changes](../common-changes.md) explain shared access, error handling, metadata, and example conventions.

## Required changes

| Attribute / behavior | Gophish 0.12.1 | Ruphish / client change |
| --- | --- | --- |
| `id`, `username`, `api_key` | Integer and strings. | Response names/types remain. Account responses still contain API keys; do not log or display them unnecessarily. |
| `role` | Object containing string `slug`, `name`, `description`. | Same response shape. Write requests use a role slug string instead. |
| `password_change_required`, `account_locked` | Booleans. | Same fields; account restrictions are enforced for API-key access. |
| `last_login` | RFC 3339 string, including zero time for never logged in. | RFC 3339 string or null; update parsers to accept null. |
| Write attributes | String username/password/role and boolean reset/lock flags. | Same fields. Send complete intended values; omitted strings become empty and omitted flags false. Password length is 8–1024 UTF-8 bytes. |
| Self password replacement | A nonempty password could replace your own password using the API key. | This endpoint rejects replacing your own password. An administrator can reset another account; use the account UI for your own password change. |
| Privileges | Role changes restricted; lock/reset fields were writable in the handler. | Ordinary accounts cannot change their role, lock state, or password-setup requirement. |
| Empty password | Preserves existing hash. | Still preserves it; send an empty string for an unrelated account edit. |

## Gophish request

`PUT /api/users/{id}`

```json
{
  "username": "automation",
  "password": "",
  "role": "user",
  "password_change_required": false,
  "account_locked": false
}
```

## Gophish response

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
  "last_login": "0001-01-01T00:00:00Z"
}
```

## Ruphish request

Request type: `auth::UserEdit`. Save this body as `request.json`:

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

## Ruphish response

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

## Behavior and verification

Editing requires `modify_objects`; editing another account also requires `modify_system`. Keep a password field empty unless performing an authorized reset of another account. Protections prevent removing the last available administrator.

Check the account model with `last_login: null`. Exercise permission failures with a representative ordinary account, and keep all returned API keys out of logs.

Current contract: [Update an account](../../../api/users-and-api-keys.md). Comparison source: [Gophish handler at the pinned version](https://github.com/gophish/gophish/blob/95618469799295e2c0fec980805a2dfbb818816b/controllers/api/user.go).
