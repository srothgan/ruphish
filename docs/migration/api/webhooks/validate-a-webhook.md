---
title: Validate a webhook
description: Migrate POST /api/webhooks/{id}/validate to the current Ruphish contract.
sidebar:
  order: 254
prev: {"label": "Choose endpoint comparisons", "link": "/docs/migration/api/"}
next: {"label": "Verify and cut over", "link": "/docs/migration/verification-and-rollback/"}
---

Update webhook permissions, secret handling, and revision-aware writes.

| | Gophish 0.12.1 | Ruphish |
| --- | --- | --- |
| Operation | `POST /api/webhooks/{id}/validate` | `POST /api/webhooks/{id}/validate` |
| Success | `200 OK` | `200 OK` |
| Response type | `models.Webhook` | `integrations::Webhook` |

Use a Bearer API key. The [common changes](../common-contract-changes.md) explain shared access, error handling, metadata, and example conventions.

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
| Network action | Signed `{"success":true}` request; success returned the stored webhook, including its secret. | Same validation payload; success returns the current secret-free resource. 2xx receiver response required. |

## Gophish request

`POST /api/webhooks/{id}/validate`

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
curl --request POST "$RUPHISH_URL/api/webhooks/1/validate" \
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

Reads and deletion remain available while the feature is disabled; saves and network actions return `409 FEATURE_DISABLED`. Editing a configuration cancels queued work for its previous revision, but a request already in flight may finish. Validation sends an actual request to the saved receiver. It does not create an ordinary campaign event or prove subsequent event processing.

Verify your client preserves the saved secret when editing unrelated fields and handles disabled-feature and stale-revision responses. Test validation only against a controlled receiver.

Current contract: [Validate a webhook](../../../api/webhooks.md). Comparison source: [Gophish handler at the pinned version](https://github.com/gophish/gophish/blob/95618469799295e2c0fec980805a2dfbb818816b/controllers/api/webhook.go).
