---
title: Authentication
description: Bearer API keys, account permissions, and credential rotation.
sidebar:
  order: 1
---

Authenticate each request with the API key of a Ruphish application account. The key has that account’s current permissions.

## Bearer header

```http
Authorization: Bearer YOUR_API_KEY
```

Set the base URL to your Ruphish application origin, without a trailing slash. The examples use POSIX shell syntax and curl.

```sh
export RUPHISH_URL="https://ruphish.example.com"
export RUPHISH_API_KEY="YOUR_API_KEY"
```

Replace `YOUR_API_KEY` with your account’s key. Use a secret store for deployed integrations. Send requests over HTTPS; avoid including the key in URLs or logs.

Example authenticated request, with no body:

```sh
curl --request GET "$RUPHISH_URL/api/groups/" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
```

A successful response for an installation with no groups is `200 OK`, `application/json`:

```json
[]
```

An invalid key returns `401 Unauthorized`:

```json
{"success":false,"message":"Invalid API Key","data":null,"code":"AUTH_API_KEY_INVALID"}
```

## Permissions

| Operation | Required permission |
| --- | --- |
| Resource and campaign reads; results, exports, overview | `view_objects` |
| Resource writes; imports and previews; verification and integration actions; campaign preparation, tests, launch, completion, and corrections | `modify_objects` |
| Company policy, bulk closure, account administration, and the starter catalog | `modify_system`, plus applicable account-edit permissions |

Resources and campaigns are shared across the installation. Creator attribution does not make a resource private. A key cannot bypass a locked account, mandatory password setup, revision checks, disabled features, or state restrictions.

See [account endpoints](users-and-api-keys.md) for access to your own account, account administration, and API-key rotation. Rotating a key invalidates the previous key immediately.
