---
title: List accounts
description: Migrate GET /api/users/ to the current Ruphish contract.
sidebar:
  order: 280
prev: {"label": "Choose endpoint comparisons", "link": "/docs/migration/api/"}
next: {"label": "Verify and cut over", "link": "/docs/migration/verification-and-rollback/"}
---

Update account handling for nullable login times and current privilege rules.

| | Gophish 0.12.1 | Ruphish |
| --- | --- | --- |
| Operation | `GET /api/users/` | `GET /api/users/` |
| Success | `200 OK` | `200 OK` |
| Response type | `[]models.User` | `Vec<auth::User>` |

Use a Bearer API key. The [common changes](../common-contract-changes.md) explain shared access, error handling, metadata, and example conventions.

## Required changes

| Attribute / behavior | Gophish 0.12.1 | Ruphish / client change |
| --- | --- | --- |
| `id`, `username`, `api_key` | Integer and strings. | Response names/types remain. Account responses still contain API keys; do not log or display them unnecessarily. |
| `role` | Object containing string `slug`, `name`, `description`. | Same response shape. Write requests use a role slug string instead. |
| `password_change_required`, `account_locked` | Booleans. | Same fields; account restrictions are enforced for API-key access. |
| `last_login` | RFC 3339 string, including zero time for never logged in. | RFC 3339 string or null; update parsers to accept null. |

## Gophish request

`GET /api/users/`

No request body.

## Gophish response

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
    "last_login": "0001-01-01T00:00:00Z"
  }
]
```

## Ruphish request

No request body.

```sh
curl --request GET "$RUPHISH_URL/api/users/" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

## Ruphish response

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

## Behavior and verification

Listing and creation require `modify_system`. Reading another account requires `modify_system`; reading your own account does not. Creation still returns `200 OK`.

Check the account model with `last_login: null`. Exercise permission failures with a representative ordinary account, and keep all returned API keys out of logs.

Current contract: [List accounts](../../../api/accounts-and-keys.md). Comparison source: [Gophish handler at the pinned version](https://github.com/gophish/gophish/blob/95618469799295e2c0fec980805a2dfbb818816b/controllers/api/user.go).
