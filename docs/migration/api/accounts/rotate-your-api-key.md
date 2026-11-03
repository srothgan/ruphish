---
title: Rotate your API key
description: Migrate POST /api/reset to the current Ruphish contract.
sidebar:
  order: 285
prev: {"label": "Choose endpoint comparisons", "link": "/docs/migration/api/"}
next: {"label": "Verify and cut over", "link": "/docs/migration/verification-and-rollback/"}
---

Keep the rotation operation and switch clients to the returned replacement key.

| | Gophish 0.12.1 | Ruphish |
| --- | --- | --- |
| Operation | `POST /api/reset` | `POST /api/reset` |
| Success | `200 OK` | `200 OK` |
| Response type | `models.Response` | `common::Envelope<String>` |

Use a Bearer API key. The [common changes](../common-contract-changes.md) explain shared access, error handling, metadata, and example conventions.

## Required changes

| Attribute / behavior | Gophish 0.12.1 | Ruphish / client change |
| --- | --- | --- |
| Request | Authenticated POST, no resource ID. | Same account-scoped operation; send the current Bearer key and empty form. |
| Response | `success` boolean, `message` string, `data` replacement key string. | Same types. Message punctuation differs; consume `data`, not exact message text. |
| Credential lifecycle | Replaces current API key. | The old key stops working after successful rotation. Do not retry with it as if rotation were read-only. |

## Gophish request

`POST /api/reset`

No JSON body. Authenticate the POST with the account's current API key.

## Gophish response

```json
{
  "success": true,
  "message": "API Key successfully reset!",
  "data": "example-replacement-api-key"
}
```

## Ruphish request

Request type: `empty URL-encoded form, with Bearer authentication`. Send an empty URL-encoded form with the current Bearer key, as shown below.

```sh
curl --request POST "$RUPHISH_URL/api/reset" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header "Content-Type: application/x-www-form-urlencoded" \
  --data ""
```

## Ruphish response

```json
{
  "success": true,
  "message": "API Key successfully reset",
  "data": "example-replacement-api-key"
}
```

## Behavior and verification

Store the returned key before switching the client. If the response is lost, the old key may already be invalid; use authorized account/key recovery rather than blindly repeating the request.

Verify that the new key authenticates and the replaced key fails, using a disposable account for the test.

Current contract: [Rotate your API key](../../../api/accounts-and-keys.md). Comparison source: [Gophish handler at the pinned version](https://github.com/gophish/gophish/blob/95618469799295e2c0fec980805a2dfbb818816b/controllers/api/reset.go).
