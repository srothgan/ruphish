---
title: Delete a webhook
description: Migrate DELETE /api/webhooks/{id} to the current Ruphish contract.
sidebar:
  order: 255
prev: {"label": "Choose endpoint comparisons", "link": "/docs/migration/api/"}
next: {"label": "Verify and cut over", "link": "/docs/migration/verification-and-rollback/"}
---

Update webhook permissions, secret handling, and revision-aware writes.

| | Gophish 0.12.1 | Ruphish |
| --- | --- | --- |
| Operation | `DELETE /api/webhooks/{id}` | `DELETE /api/webhooks/{id}` |
| Success | `200 OK` | `200 OK` |
| Response type | `models.Response` | `serde_json::Value` |

Use a Bearer API key. The [common changes](../common-contract-changes.md) explain shared access, error handling, metadata, and example conventions.

## Required changes

| Attribute / behavior | Gophish 0.12.1 | Ruphish / client change |
| --- | --- | --- |
| `revision` | Absent. | Integer. PUT sends it in JSON; DELETE sends quoted `If-Match`. |
| Permissions | System-administrator permission required. | Reads require `view_objects`; writes/actions require `modify_objects`. Administrator controls the global feature switch. |
| Success response | Envelope: `success`, `message`, `data`. | Only `{"success":true}`. Do not require message/data. |

## Gophish request

`DELETE /api/webhooks/{id}`

No request body.

## Gophish response

```json
{
  "success": true,
  "message": "Webhook deleted Successfully!",
  "data": null
}
```

## Ruphish request

No request body.

```sh
curl --request DELETE "$RUPHISH_URL/api/webhooks/1" \
  --header "Authorization: Bearer $RUPHISH_API_KEY" \
  --header 'If-Match: "2"'
```

## Ruphish response

```json
{
  "success": true
}
```

## Behavior and verification

Reads and deletion remain available while the feature is disabled; saves and network actions return `409 FEATURE_DISABLED`. Editing a configuration cancels queued work for its previous revision, but a request already in flight may finish.

Verify your client preserves the saved secret when editing unrelated fields and handles disabled-feature and stale-revision responses. Test validation only against a controlled receiver.

Current contract: [Delete a webhook](../../../api/webhooks.md). Comparison source: [Gophish handler at the pinned version](https://github.com/gophish/gophish/blob/95618469799295e2c0fec980805a2dfbb818816b/controllers/api/webhook.go).
