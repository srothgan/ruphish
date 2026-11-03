---
title: Read a webhook
description: Migrate GET /api/webhooks/{id} to the current Ruphish contract.
sidebar:
  order: 251
prev: {"label": "Choose endpoint comparisons", "link": "/docs/migration/api/"}
next: {"label": "Verify and cut over", "link": "/docs/migration/verification-and-rollback/"}
---

Update webhook permissions, secret handling, and revision-aware writes.

| | Gophish 0.12.1 | Ruphish |
| --- | --- | --- |
| Operation | `GET /api/webhooks/{id}` | `GET /api/webhooks/{id}` |
| Success | `200 OK` | `200 OK` |
| Response type | `models.Webhook` | `integrations::Webhook` |

Use a Bearer API key. The [common changes](../common-changes.md) explain shared access, error handling, metadata, and example conventions.

## Required changes

| Attribute / behavior | Gophish 0.12.1 | Ruphish / client change |
| --- | --- | --- |
| `id`, `name`, `url`, `is_active` | Integer, string, string, boolean. | Response names/types remain. Keep `id` in the URL, not a write body. |
| `secret` | String returned by reads and saves. | Write-only optional string. Required on creation; omitted/null preserves on update; empty replacement rejected. |
| `has_secret` | Absent. | Boolean credential-presence flag, never the secret value. |
| `revision` | Absent. | Integer. PUT sends it in JSON; DELETE sends quoted `If-Match`. |
| Attribution and `history` | Absent. | Nullable creator/editor metadata and an array of recent attempt records. See the current field tables. |
| Permissions | System-administrator permission required. | Reads require `view_objects`; writes/actions require `modify_objects`. Administrator controls the global feature switch. |
| Unknown write fields | Decoded into a Go model. | Rejected. Do not echo `id`, `has_secret`, attribution, or `history` into PUT. |

## Gophish request

`GET /api/webhooks/{id}`

No request body.

## Gophish response

```json
{
  "id": 1,
  "name": "Event receiver",
  "url": "https://events.example.com/ruphish",
  "is_active": true,
  "secret": "example-signing-secret"
}
```

## Ruphish request

No request body.

```sh
curl --request GET "$RUPHISH_URL/api/webhooks/1" \
  --header "Authorization: Bearer $RUPHISH_API_KEY"
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
  "id": 1,
  "name": "Event receiver",
  "url": "https://events.example.com/ruphish",
  "is_active": true,
  "revision": 1,
  "has_secret": true,
  "history": []
}
```

## Behavior and verification

Reads and deletion remain available while the feature is disabled; saves and network actions return `409 FEATURE_DISABLED`. Editing a configuration cancels queued work for its previous revision, but a request already in flight may finish.

Verify your client preserves the saved secret when editing unrelated fields and handles disabled-feature and stale-revision responses. Test validation only against a controlled receiver.

Current contract: [Read a webhook](../../../api/webhooks.md). Comparison source: [Gophish handler at the pinned version](https://github.com/gophish/gophish/blob/95618469799295e2c0fec980805a2dfbb818816b/controllers/api/webhook.go).
